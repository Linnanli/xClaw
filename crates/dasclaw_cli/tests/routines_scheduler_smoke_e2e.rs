//! PR2 GUI-only blind-spot: `dasclaw_routines` public-surface contract.
//!
//! The only in-tree consumer of [`dasclaw_routines`] today is
//! `desktop-client/ironclaw` (e.g. `src/routines/scheduler.rs:971-1173`).
//! The headless CLI never links the crate in production, so any silent
//! breakage in `Routine` / `Trigger` / `RoutineAction` / `RunStatus` /
//! `RoutineGuardrails` escapes the ADR-153 §1 CLI test matrix.
//!
//! ## Why no `Scheduler` test
//!
//! The GUI's `Scheduler` lives in `desktop-client/ironclaw/src/routines/
//! scheduler.rs`, **not** in this crate — `dasclaw_routines` exposes only
//! the data types (`Routine`, `Trigger`, `RoutineAction`, `RunStatus`,
//! `RoutineRun`, `RoutineGuardrails`, `NotifyConfig`,
//! `RoutineDisplayStatus`, `RoutineVerificationStatus`) plus
//! `routine_verification_fingerprint` / `reset_routine_verification_state`
//! / `content_hash`. Promoting `Scheduler` to a crate is tracked elsewhere
//! (F4.2 follow-up; the lib.rs header lists the remaining sub-modules
//! pending de-coupling from desktop modules).
//!
//! So this file pins the state-machine pieces that ARE crate-public:
//!
//! 1. `Trigger` round-trip through `from_db` / `to_config_json` /
//!    `type_tag` covering every variant (`cron` / `event` /
//!    `system_event` / `webhook` / `manual`).
//! 2. `RoutineAction` round-trip for `lightweight` + `full_job`, with
//!    the `max_tool_rounds` clamp invariant (`MAX_TOOL_ROUNDS_LIMIT`).
//! 3. `RunStatus` Display + FromStr round-trip + unknown-status error.
//! 4. `RoutineDisplayStatus` and `RoutineVerificationStatus`
//!    `as_str` mappings (UI-facing labels).

use std::str::FromStr;
use std::time::Duration;

use dasclaw_routines::error::RoutineError;
use dasclaw_routines::routine::{
    MAX_TOOL_ROUNDS_LIMIT, NotifyConfig, RoutineAction, RoutineDisplayStatus, RoutineGuardrails,
    RoutineVerificationStatus, RunStatus, Trigger,
};
use serde_json::json;

/// `req_dasclaw_cli_routines_pr2_trigger_round_trip_cron` — cron trigger
/// survives DB serialize/parse, including the timezone field.
#[test]
fn req_dasclaw_cli_routines_pr2_trigger_round_trip_cron() {
    let original = Trigger::Cron {
        schedule: "0 9 * * MON-FRI".into(),
        timezone: Some("America/New_York".into()),
    };
    assert_eq!(original.type_tag(), "cron");

    let cfg = original.to_config_json();
    assert_eq!(cfg["schedule"], json!("0 9 * * MON-FRI"));
    assert_eq!(cfg["timezone"], json!("America/New_York"));

    let parsed = Trigger::from_db("cron", cfg).expect("cron must round-trip");
    match parsed {
        Trigger::Cron { schedule, timezone } => {
            assert_eq!(schedule, "0 9 * * MON-FRI");
            assert_eq!(timezone.as_deref(), Some("America/New_York"));
        }
        other => panic!("expected Cron, got {other:?}"),
    }
}

/// `req_dasclaw_cli_routines_pr2_trigger_round_trip_event` — event
/// trigger round-trip with optional channel set.
#[test]
fn req_dasclaw_cli_routines_pr2_trigger_round_trip_event() {
    let original = Trigger::Event {
        channel: Some("telegram".into()),
        pattern: "^/digest".into(),
    };
    assert_eq!(original.type_tag(), "event");

    let cfg = original.to_config_json();
    let parsed = Trigger::from_db("event", cfg).expect("event must round-trip");
    match parsed {
        Trigger::Event { channel, pattern } => {
            assert_eq!(channel.as_deref(), Some("telegram"));
            assert_eq!(pattern, "^/digest");
        }
        other => panic!("expected Event, got {other:?}"),
    }
}

/// `req_dasclaw_cli_routines_pr2_trigger_round_trip_system_event` —
/// system_event with filters survives round-trip; missing `event_type`
/// errors with `MissingField`.
#[test]
fn req_dasclaw_cli_routines_pr2_trigger_round_trip_system_event() {
    let original = Trigger::SystemEvent {
        source: "github".into(),
        event_type: "issue.opened".into(),
        filters: [("label".to_string(), "bug".to_string())]
            .into_iter()
            .collect(),
    };
    assert_eq!(original.type_tag(), "system_event");

    let parsed = Trigger::from_db("system_event", original.to_config_json())
        .expect("system_event must round-trip");
    match parsed {
        Trigger::SystemEvent {
            source,
            event_type,
            filters,
        } => {
            assert_eq!(source, "github");
            assert_eq!(event_type, "issue.opened");
            assert_eq!(filters.get("label").map(String::as_str), Some("bug"));
        }
        other => panic!("expected SystemEvent, got {other:?}"),
    }

    let bad = Trigger::from_db("system_event", json!({"source": "github"}));
    assert!(
        matches!(&bad, Err(RoutineError::MissingField { field, .. }) if field == "event_type"),
        "missing event_type must surface MissingField, got {bad:?}"
    );
}

/// `req_dasclaw_cli_routines_pr2_trigger_round_trip_webhook_and_manual`
/// — covers the last two trigger variants in one pass.
#[test]
fn req_dasclaw_cli_routines_pr2_trigger_round_trip_webhook_and_manual() {
    let webhook = Trigger::Webhook {
        path: Some("hook/x".into()),
        secret: Some("s".into()),
    };
    assert_eq!(webhook.type_tag(), "webhook");
    let parsed =
        Trigger::from_db("webhook", webhook.to_config_json()).expect("webhook must round-trip");
    match parsed {
        Trigger::Webhook { path, secret } => {
            assert_eq!(path.as_deref(), Some("hook/x"));
            assert_eq!(secret.as_deref(), Some("s"));
        }
        other => panic!("expected Webhook, got {other:?}"),
    }

    let manual = Trigger::Manual;
    assert_eq!(manual.type_tag(), "manual");
    let parsed =
        Trigger::from_db("manual", manual.to_config_json()).expect("manual must round-trip");
    assert!(matches!(parsed, Trigger::Manual));
}

/// `req_dasclaw_cli_routines_pr2_trigger_unknown_type_errors` — unknown
/// trigger_type must surface a typed error, not a panic.
#[test]
fn req_dasclaw_cli_routines_pr2_trigger_unknown_type_errors() {
    let err = Trigger::from_db("not_a_trigger", json!({}));
    assert!(
        matches!(err, Err(RoutineError::UnknownTriggerType { ref trigger_type }) if trigger_type == "not_a_trigger"),
        "unknown trigger_type must surface UnknownTriggerType, got {err:?}"
    );
}

/// `req_dasclaw_cli_routines_pr2_action_round_trip_lightweight_clamps_rounds`
/// — lightweight action round-trip + the `max_tool_rounds` clamp
/// invariant. A value above `MAX_TOOL_ROUNDS_LIMIT` must be clamped
/// down silently to the limit (the constant governs the runaway-loop
/// guardrail; if it ever drifts, downstream cost projections break).
#[test]
fn req_dasclaw_cli_routines_pr2_action_round_trip_lightweight_clamps_rounds() {
    let cfg = json!({
        "prompt": "summarize",
        "context_paths": ["context/a.md"],
        "max_tokens": 1024,
        "use_tools": true,
        "max_tool_rounds": 10_000u64,
    });
    let parsed = RoutineAction::from_db("lightweight", cfg).expect("lightweight must round-trip");
    match parsed {
        RoutineAction::Lightweight {
            prompt,
            context_paths,
            max_tokens,
            use_tools,
            max_tool_rounds,
        } => {
            assert_eq!(prompt, "summarize");
            assert_eq!(context_paths, vec!["context/a.md".to_string()]);
            assert_eq!(max_tokens, 1024);
            assert!(use_tools);
            assert_eq!(
                max_tool_rounds, MAX_TOOL_ROUNDS_LIMIT,
                "max_tool_rounds must be clamped down to MAX_TOOL_ROUNDS_LIMIT"
            );
        }
        other => panic!("expected Lightweight, got {other:?}"),
    }
}

/// `req_dasclaw_cli_routines_pr2_action_round_trip_full_job` — full_job
/// action round-trip + required-field error path.
#[test]
fn req_dasclaw_cli_routines_pr2_action_round_trip_full_job() {
    let job = RoutineAction::FullJob {
        title: "nightly".into(),
        description: "audit".into(),
        max_iterations: 7,
    };
    assert_eq!(job.type_tag(), "full_job");

    let parsed =
        RoutineAction::from_db("full_job", job.to_config_json()).expect("full_job must round-trip");
    match parsed {
        RoutineAction::FullJob {
            title,
            description,
            max_iterations,
        } => {
            assert_eq!(title, "nightly");
            assert_eq!(description, "audit");
            assert_eq!(max_iterations, 7);
        }
        other => panic!("expected FullJob, got {other:?}"),
    }

    let bad = RoutineAction::from_db("full_job", json!({"title": "x"}));
    assert!(
        matches!(&bad, Err(RoutineError::MissingField { field, .. }) if field == "description"),
        "missing description must surface MissingField, got {bad:?}"
    );
}

/// `req_dasclaw_cli_routines_pr2_run_status_display_from_str_round_trip`
/// — `RunStatus` is the smallest state machine in the crate. Pin its
/// Display + FromStr contract so the DB/UI side of the pipeline can't
/// silently drift.
#[test]
fn req_dasclaw_cli_routines_pr2_run_status_display_from_str_round_trip() {
    for status in [
        RunStatus::Running,
        RunStatus::Ok,
        RunStatus::Attention,
        RunStatus::Failed,
    ] {
        let s = status.to_string();
        let parsed = RunStatus::from_str(&s).expect("known RunStatus must parse");
        assert_eq!(
            parsed, status,
            "Display + FromStr must round-trip for {status:?}"
        );
    }

    let bad = RunStatus::from_str("not_a_status");
    assert!(
        matches!(bad, Err(RoutineError::UnknownRunStatus { ref status }) if status == "not_a_status"),
        "unknown run status must surface UnknownRunStatus, got {bad:?}"
    );
}

/// `req_dasclaw_cli_routines_pr2_display_status_labels_stable` —
/// `RoutineDisplayStatus::as_str` powers the UI badge label; the labels
/// are part of the user-facing contract and must not silently change.
#[test]
fn req_dasclaw_cli_routines_pr2_display_status_labels_stable() {
    assert_eq!(RoutineDisplayStatus::Disabled.as_str(), "disabled");
    assert_eq!(RoutineDisplayStatus::Running.as_str(), "running");
    assert_eq!(RoutineDisplayStatus::Unverified.as_str(), "unverified");
    assert_eq!(RoutineDisplayStatus::Failing.as_str(), "failing");
    assert_eq!(RoutineDisplayStatus::Attention.as_str(), "attention");
    assert_eq!(RoutineDisplayStatus::Active.as_str(), "active");

    assert_eq!(RoutineVerificationStatus::Verified.as_str(), "verified");
    assert_eq!(RoutineVerificationStatus::Unverified.as_str(), "unverified");
}

/// `req_dasclaw_cli_routines_pr2_guardrails_and_notify_defaults` —
/// `RoutineGuardrails::default` and `NotifyConfig::default` are the
/// safe-defaults consumed by every routine that doesn't override them.
/// Pin the values so cross-crate "I assumed cooldown was 5 min" drift
/// surfaces here.
#[test]
fn req_dasclaw_cli_routines_pr2_guardrails_and_notify_defaults() {
    let g = RoutineGuardrails::default();
    assert_eq!(g.cooldown, Duration::from_secs(300));
    assert_eq!(g.max_concurrent, 1);
    assert!(g.dedup_window.is_none());

    let n = NotifyConfig::default();
    assert!(n.channel.is_none());
    assert!(n.user.is_none());
    assert!(n.on_attention, "default must alert on actionable output");
    assert!(n.on_failure, "default must alert on errors");
    assert!(!n.on_success, "default must stay quiet on no-finding runs");
}
