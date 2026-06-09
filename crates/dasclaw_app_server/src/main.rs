//! Line-delimited stdio JSON-RPC entrypoint for the Phase 1 app-server.
//!
//! Each non-empty stdin line is treated as one JSON-RPC request. Each response
//! is written as one stdout line so Electron can supervise this process as a
//! simple sidecar before we commit to a longer-lived socket transport.

use std::io;

use dasclaw_app_server::{AppServer, run_stdio_server};
use dasclaw_app_server_protocol::{
    CapabilitiesListResponse, ClientInfo, HealthCheckParams, HealthCheckResponse, InitializeParams,
    InitializeResponse, LifecycleStatusResponse, ProtocolSchemaResponse, ProtocolVersion,
    ThreadCreateParams, ThreadCreateResponse, ThreadListResponse, ThreadReadParams,
    ThreadReadResponse, TransportKind, TurnCancelParams, TurnCancelResponse, TurnListParams,
    TurnListResponse, TurnReadParams, TurnReadResponse, TurnStartParams, TurnStartResponse,
};
use serde::Serialize;

fn main() {
    match parse_run_mode(std::env::args().skip(1)) {
        Ok(RunMode::Stdio) => {
            if let Err(error) =
                run_stdio_server(io::BufReader::new(io::stdin()), io::stdout().lock())
            {
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
    let mut server = AppServer::new();
    let health = server.health_check(HealthCheckParams {
        include_details: true,
    });
    match serde_json::to_string(&health) {
        Ok(line) => println!("{line}"),
        Err(error) => {
            eprintln!("dasclaw-app-server failed to serialize health: {error}");
            std::process::exit(1);
        }
    }
}

fn print_capabilities_once() {
    let mut server = AppServer::new();
    match serde_json::to_string(&server.capabilities()) {
        Ok(line) => println!("{line}"),
        Err(error) => {
            eprintln!("dasclaw-app-server failed to serialize capabilities: {error}");
            std::process::exit(1);
        }
    }
}

fn print_schema_once() {
    let server = AppServer::new();
    match serde_json::to_string(&server.protocol_schema()) {
        Ok(line) => println!("{line}"),
        Err(error) => {
            eprintln!("dasclaw-app-server failed to serialize protocol schema: {error}");
            std::process::exit(1);
        }
    }
}

fn print_self_check() {
    match self_check_report() {
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

fn self_check_report() -> Result<SelfCheckReport, dasclaw_app_server::AppServerError> {
    let mut server = AppServer::new();
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
    })?;
    let pending_notifications = server.drain_notifications().len();
    let thread_create = server.thread_create(ThreadCreateParams {
        title: Some("self-check".to_string()),
        workspace_root: None,
    })?;
    let thread_list = server.thread_list()?;
    let thread_read = server.thread_read(ThreadReadParams {
        thread_id: thread_create.thread_id.clone(),
    })?;
    let turn_start = server.turn_start(TurnStartParams {
        thread_id: thread_create.thread_id.clone(),
        prompt: "self-check prompt exercises runtime bridge session bookkeeping".to_string(),
    })?;
    let turn_cancel = server.turn_cancel(TurnCancelParams {
        thread_id: thread_create.thread_id.clone(),
        turn_id: turn_start.turn_id.clone(),
    })?;
    let turn_list = server.turn_list(TurnListParams {
        thread_id: thread_create.thread_id.clone(),
    })?;
    let turn_read = server.turn_read(TurnReadParams {
        thread_id: thread_create.thread_id.clone(),
        turn_id: turn_start.turn_id.clone(),
    })?;
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
            thread_create,
            thread_list,
            thread_read,
            turn_start,
            turn_cancel,
            turn_list,
            turn_read,
            pending_notifications: session_notifications,
        },
    })
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
    thread_create: ThreadCreateResponse,
    thread_list: ThreadListResponse,
    thread_read: ThreadReadResponse,
    turn_start: TurnStartResponse,
    turn_cancel: TurnCancelResponse,
    turn_list: TurnListResponse,
    turn_read: TurnReadResponse,
    pending_notifications: usize,
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;
    use std::sync::{Arc, Mutex};
    use std::thread;
    use std::time::Duration;

    use dasclaw_app_server::{
        RuntimeBridge, RuntimeBridgeError, RuntimeTurnCancelRequest, RuntimeTurnStartRequest,
    };
    use serde_json::Value;

    use dasclaw_app_server::run_stdio_server_with_app_server;

    use super::{RunMode, parse_run_mode, run_stdio_server};

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
        assert_eq!(lines.len(), 4);

        let initializing: Value =
            serde_json::from_str(lines[0]).expect("initializing notification should be JSON");
        let ready: Value =
            serde_json::from_str(lines[1]).expect("ready notification should be JSON");
        let capabilities: Value =
            serde_json::from_str(lines[2]).expect("capability notification should be JSON");
        let response: Value = serde_json::from_str(lines[3]).expect("response should be JSON");

        assert_eq!(initializing["method"], "lifecycle/changed");
        assert_eq!(initializing["params"]["lifecycle"]["state"], "initializing");
        assert_eq!(ready["method"], "lifecycle/changed");
        assert_eq!(ready["params"]["lifecycle"]["state"], "ready");
        assert_eq!(capabilities["method"], "capabilities/changed");
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
        assert_eq!(lines.len(), 5);

        let init_response: Value = serde_json::from_str(lines[3]).expect("init response JSON");
        let health_response: Value = serde_json::from_str(lines[4]).expect("health response JSON");

        assert_eq!(init_response["result"]["lifecycle"]["state"], "ready");
        assert_eq!(health_response["id"], "health");
        assert_eq!(health_response["result"]["ok"], true);
        assert_eq!(health_response["result"]["lifecycle"]["state"], "ready");
    }

    #[test]
    fn stdio_loop_can_run_with_injected_app_server_runtime_bridge() {
        let initialize = r#"{"jsonrpc":"2.0","id":"init","method":"initialize","params":{"client":{"name":"open-cowork","version":"0.0.0","transport":"stdio"},"protocolVersion":{"major":0,"minor":1,"patch":0},"requestedCapabilities":[]}}"#;
        let create = r#"{"jsonrpc":"2.0","id":"thread","method":"thread/create","params":{"title":"Draft"}}"#;
        let start = r#"{"jsonrpc":"2.0","id":"turn","method":"turn/start","params":{"threadId":"thread_1","prompt":"hello"}}"#;
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
        assert!(
            values
                .iter()
                .any(|value| { value["id"] == "turn" && value["result"]["status"] == "pending" })
        );
    }

    #[test]
    fn stdio_loop_supports_codex_v2_chat_subset_transcript() {
        let initialize = r#"{"jsonrpc":"2.0","id":"init","method":"initialize","params":{"client":{"name":"codex","version":"2.0.0","transport":"stdio"},"protocolVersion":{"major":0,"minor":1,"patch":0},"requestedCapabilities":["codex_app_server_v2"]}}"#;
        let thread_start =
            r#"{"jsonrpc":"2.0","id":"thread","method":"thread/start","params":{"title":"Draft"}}"#;
        let turn_start = r#"{"jsonrpc":"2.0","id":"turn","method":"turn/start","params":{"threadId":"thread_1","input":"hello"}}"#;
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
            .find(|profile| profile["id"] == "codex_app_server_v2")
            .expect("Codex v2 profile should be advertised");
        let methods = values
            .iter()
            .filter_map(|value| value.get("method").and_then(Value::as_str))
            .collect::<Vec<_>>();

        assert_eq!(profile["scope"], "chat_session_subset");
        assert!(
            values.iter().any(|value| {
                value["id"] == "thread" && value["result"]["threadId"] == "thread_1"
            })
        );
        assert!(
            values
                .iter()
                .any(|value| { value["id"] == "turn" && value["result"]["turnId"] == "turn_1" })
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
    fn stdio_loop_emits_codex_v2_failure_item_terminal_and_error_events() {
        let initialize = r#"{"jsonrpc":"2.0","id":"init","method":"initialize","params":{"client":{"name":"codex","version":"2.0.0","transport":"stdio"},"protocolVersion":{"major":0,"minor":1,"patch":0},"requestedCapabilities":["codex_app_server_v2"]}}"#;
        let thread_start =
            r#"{"jsonrpc":"2.0","id":"thread","method":"thread/start","params":{"title":"Draft"}}"#;
        let turn_start = r#"{"jsonrpc":"2.0","id":"turn","method":"turn/start","params":{"threadId":"thread_1","input":"hello"}}"#;
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

        assert!(values.iter().any(|value| value["method"] == "turn/failed"));
        assert!(values.iter().any(|value| {
            value["method"] == "item/completed" && value["params"]["status"] == "failed"
        }));
        assert!(values.iter().any(|value| {
            value["method"] == "error" && value["params"]["message"] == "runtime failed"
        }));
        let item_completed_position = values
            .iter()
            .position(|value| value["method"] == "item/completed")
            .expect("item/completed should be present");
        let turn_failed_position = values
            .iter()
            .position(|value| value["method"] == "turn/failed")
            .expect("turn/failed should be present");
        let error_position = values
            .iter()
            .position(|value| value["method"] == "error")
            .expect("error should be present");
        assert!(item_completed_position < turn_failed_position);
        assert!(turn_failed_position < error_position);
    }

    #[test]
    fn stdio_loop_emits_codex_v2_interrupt_item_terminal_events() {
        let initialize = r#"{"jsonrpc":"2.0","id":"init","method":"initialize","params":{"client":{"name":"codex","version":"2.0.0","transport":"stdio"},"protocolVersion":{"major":0,"minor":1,"patch":0},"requestedCapabilities":["codex_app_server_v2"]}}"#;
        let thread_start =
            r#"{"jsonrpc":"2.0","id":"thread","method":"thread/start","params":{"title":"Draft"}}"#;
        let turn_start = r#"{"jsonrpc":"2.0","id":"turn","method":"turn/start","params":{"threadId":"thread_1","input":"hello"}}"#;
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

        assert!(
            values
                .iter()
                .any(|value| value["method"] == "turn/cancelled")
        );
        assert!(values.iter().any(|value| {
            value["method"] == "item/completed" && value["params"]["status"] == "cancelled"
        }));
        assert!(values.iter().any(|value| {
            value["id"] == "interrupt" && value["result"]["status"] == "cancelled"
        }));
        let item_completed_position = values
            .iter()
            .position(|value| value["method"] == "item/completed")
            .expect("item/completed should be present");
        let turn_cancelled_position = values
            .iter()
            .position(|value| value["method"] == "turn/cancelled")
            .expect("turn/cancelled should be present");
        assert!(item_completed_position < turn_cancelled_position);
    }

    #[test]
    fn stdio_loop_streams_runtime_notifications_after_response_without_another_request() {
        let initialize = r#"{"jsonrpc":"2.0","id":"init","method":"initialize","params":{"client":{"name":"open-cowork","version":"0.0.0","transport":"stdio"},"protocolVersion":{"major":0,"minor":1,"patch":0},"requestedCapabilities":[]}}"#;
        let create = r#"{"jsonrpc":"2.0","id":"thread","method":"thread/create","params":{"title":"Draft"}}"#;
        let start = r#"{"jsonrpc":"2.0","id":"turn","method":"turn/start","params":{"threadId":"thread_1","prompt":"hello"}}"#;
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
            .position(|value| value["id"] == "turn" && value["result"]["status"] == "pending")
            .expect("turn/start response should be written");
        let completed_position = values
            .iter()
            .position(|value| value["method"] == "turn/completed")
            .expect("delayed turn/completed notification should be written");

        assert!(completed_position > turn_response_position);
    }

    #[test]
    fn stdio_loop_disconnects_after_notification_queue_overflow() {
        let initialize = r#"{"jsonrpc":"2.0","id":"init","method":"initialize","params":{"client":{"name":"open-cowork","version":"0.0.0","transport":"stdio"},"protocolVersion":{"major":0,"minor":1,"patch":0},"requestedCapabilities":[]}}"#;
        let create = r#"{"jsonrpc":"2.0","id":"thread","method":"thread/create","params":{"title":"overflow"}}"#;
        let start = r#"{"jsonrpc":"2.0","id":"turn","method":"turn/start","params":{"threadId":"thread_1","prompt":"overflow"}}"#;
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
        let mut server = dasclaw_app_server::AppServer::new();
        let health = server.health_check(dasclaw_app_server_protocol::HealthCheckParams {
            include_details: true,
        });
        let value = serde_json::to_value(health).expect("health response should serialize");

        assert_eq!(value["ok"], false);
        assert_eq!(value["lifecycle"]["state"], "starting");
        assert!(value["services"].is_array());
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
        let mut server = dasclaw_app_server::AppServer::new();
        let value =
            serde_json::to_value(server.capabilities()).expect("capabilities should serialize");

        assert_eq!(value["capabilities"]["protocol"]["status"], "implemented");
        assert_eq!(value["capabilities"]["logs"]["status"], "declared");
        assert_eq!(value["capabilities"]["dlpPolicy"]["status"], "declared");
    }

    #[test]
    fn schema_once_shape_is_serializable() {
        let server = dasclaw_app_server::AppServer::new();
        let value =
            serde_json::to_value(server.protocol_schema()).expect("schema should serialize");

        assert_eq!(value["protocolVersion"]["major"], 0);
        assert!(value["methods"].is_array());
        assert!(value["events"].is_array());
        assert_eq!(value["capabilities"]["protocol"]["status"], "implemented");
    }

    #[test]
    fn self_check_shape_is_serializable_and_initializes_server() {
        let report = super::self_check_report().expect("self-check should pass");
        let value = serde_json::to_value(report).expect("self-check should serialize");

        assert_eq!(value["ok"], true);
        assert_eq!(value["initialize"]["lifecycle"]["state"], "ready");
        assert_eq!(value["lifecycle"]["lifecycle"]["state"], "ready");
        assert_eq!(value["health"]["ok"], true);
        assert_eq!(value["pendingNotifications"], 3);
        assert!(value["schema"]["methods"].is_array());
        assert_eq!(value["session"]["threadCreate"]["threadId"], "thread_1");
        assert_eq!(
            value["session"]["threadList"]["threads"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            value["session"]["threadRead"]["thread"]["threadId"],
            "thread_1"
        );
        assert_eq!(value["session"]["turnStart"]["turnId"], "turn_1");
        assert_eq!(value["session"]["turnCancel"]["status"], "cancelled");
        assert_eq!(
            value["session"]["turnList"]["turns"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        assert_eq!(value["session"]["turnRead"]["turn"]["status"], "cancelled");
        assert_eq!(value["session"]["pendingNotifications"], 5);
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
