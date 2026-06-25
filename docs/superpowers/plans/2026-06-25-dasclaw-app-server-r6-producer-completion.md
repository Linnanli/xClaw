# Dasclaw App Server R6 Producer Completion Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:executing-plans` or `superpowers:subagent-driven-development` to implement this plan task-by-task. Track progress by changing each checkbox from `- [ ]` to `- [x]` only after the step and its verification pass.

**Goal:** Finish the unfinished R6 producer work from `docs/plans/dasclaw-app-server-codex-protocol-gap-matrix.md` without patch-style readiness, fake producers, or test-side adaptation that hides client/protocol bugs. The completion rule is strict: a capability may be advertised only when a normal product path creates the notification from a real runtime, provider, hook, startup, or guardian source.

**Architecture:** Keep the existing R6 protocol DTOs and delivery plumbing. Add only producer-side wiring and readiness gates. Delivery tests that manually enqueue `AppServerHookNotification` or inject `RuntimeTurnOutcome` remain useful, but they do not prove R6 completion. Every remaining R6 event must have:

- a real source in normal app-server execution,
- an explicit readiness gate in `AppServerR6Availability`,
- at least one non-synthetic regression test that fails if the real source is removed,
- a negative test proving app-server does not synthesize unsupported events.

**Execution status (2026-06-25):** Implemented and verified non-model R6 producer slices: HookRegistry `preToolUse` / `userPromptSubmit` / `postToolUse` producer paths, config/deprecation warnings, Linux resolved-sandbox generic warning, and auto-approval review guardian warning. `model/rerouted` and `model/verification` intentionally remain unavailable until a real reroute reason source and provider metadata parser exist.

Execution result:

- Completed: truthful readiness gates distinguish real producers from delivery-only queues.
- Completed: `userPromptSubmit` inbound hooks can modify or reject the runtime prompt before turn execution.
- Completed: `postToolUse` outbound hooks can modify or reject completed turn output before success notification.
- Completed: Linux resolved workspace sandbox policy can produce `warning`; non-Linux and external sandbox modes do not advertise or emit fake generic warnings.
- Completed: `AutoApprovalReviewCompleted` can produce `guardianWarning` for blocked/high-risk/timed-out review outcomes.
- Completed: docs split R6 status so the non-model producer slice is complete without marking model producers done.
- Intentionally incomplete: `model/rerouted` remains disabled because no trustworthy reroute reason source is wired.
- Intentionally incomplete: `model/verification` remains disabled because no real provider parser populates `ResponseMetadata.model_verifications`.

**Tech Stack:** Rust 2024; `dasclaw_app_server`; `dasclaw_app_server_protocol`; `dasclaw_hooks`; `dasclaw_sandboxing`; `dasclaw_runtime`; `dasclaw_llm_provider`; `cargo nextest`; `cargo check`; `cargo fmt`; `scripts/check_no_panics.py`.

---

## 0. Start Gate And Evidence

Project 4-question gate:

| Question | Answer | Consequence |
|---|---|---|
| New module / crate / file? | Yes. This plan is a new file; implementation may add a small startup warning helper. | Run reuse search before implementation. |
| Negative claims? | Yes. This plan says specific R6 producer sources are not complete. | Require semantic plus exact evidence, and symbol-level evidence when the LSP tool is available. |
| Cross-project reconciliation? | No new Codex cross-project comparison is required for this document, but it reconciles the existing R6 gap matrix against local code. | Local evidence is sufficient for this planning pass. |
| Architecture reconciliation document? | Yes. This is an R6 architecture/execution plan. | Record the evidence below and keep unsupported capability claims out of the matrix. |

Evidence gathered on 2026-06-25:

| Level | Evidence | Result |
|---|---|---|
| Level 1 semantic | `semantic_search_nodes_tool` queries for R6 model, guardian, warning, and hook producers. | Hook and sandbox warning concepts resolve to real local code (`dasclaw_hooks::HookRegistry`, `dasclaw_sandboxing::system_bwrap_warning`). Model reroute/verification and guardian warning searches did not reveal a completed real app-server producer. |
| Level 2 symbol | Attempted to use the configured LSP MCP (`execute_lsp` / `lsp.*`). The callable tool was unavailable in this Codex App session; `tool_search` returned no LSP tools and direct MCP probing required a session ID. | Implementation must rerun LSP symbol checks if available. This plan uses code-review-graph semantic results plus exact file inspection as fallback evidence. |
| Level 3 literal | `rg` over `crates/dasclaw_app_server*`, `crates/dasclaw_core`, `crates/dasclaw_runtime`, `crates/dasclaw_llm_provider`, `crates/dasclaw_sandboxing`, `crates/dasclaw_hooks`. | Protocol and notification delivery exist. Real producer gaps remain for `model/rerouted`, provider-backed `model/verification`, generic `warning`, `guardianWarning`, and hook lifecycle points beyond real `beforeToolCall`. |

Key local facts:

- `crates/dasclaw_app_server_protocol/src/lib.rs` already defines `model/rerouted`, `model/verification`, `warning`, `guardianWarning`, `hook/started`, `hook/completed`, `configWarning`, and `deprecationNotice`.
- `crates/dasclaw_app_server/src/lib.rs` already drains model outcomes and hook/warning queue entries into JSON-RPC notifications.
- `crates/dasclaw_app_server/src/app_services.rs` currently advertises `config_warnings` and `deprecation_notices` only when the config and hook notification path is ready, and keeps `warnings` / `guardian_warnings` false.
- `crates/dasclaw_app_server/src/lib.rs` currently derives `r6.model_reroutes` and `r6.model_verifications` from `RuntimeBridgeFeatures`.
- `crates/dasclaw_app_server/src/lib.rs` preserves provider `actual_model` but intentionally does not infer `model/rerouted(reason=highRiskCyberActivity)` from a requested/actual model mismatch.
- `crates/dasclaw_core/src/response_types.rs` contains `ResponseMetadata.model_verifications`, but the current real provider parsers mainly populate `actual_model`; no real parser path found that populates `model_verifications`.
- `crates/dasclaw_sandboxing/src/bwrap.rs` has `system_bwrap_warning(&SandboxPolicy)`, which is a real warning source if app-server passes the actual resolved sandbox policy.
- `crates/dasclaw_hooks/src/hook.rs` and `crates/dasclaw_hooks/src/registry.rs` support `Inbound`, `ToolCall`, `Outbound`, `ResponseTransform`, `SessionStart`, and `SessionEnd`, but app-server currently has only the real tool-call hook path clearly wired into a normal turn.

Process transparency line required in any commit created from this plan:

```text
已检查 R6 producer 是否已有，结论：协议与 delivery 已存在；模型、guardian、generic warning、剩余 hook 生命周期仍需真实 producer 或保持未广告
```

---

## 1. Definition Of Done

R6 is complete only for an event family when all rows pass:

| Event family | Completion condition | Must remain disabled when |
|---|---|---|
| `hook/started`, `hook/completed` | A real `HookRegistry` run from a normal app-server turn/thread lifecycle emits observer notifications. | Only an injected queue or direct `registry.run()` unit test exists. |
| `warning` | The warning comes from the actual resolved app-server runtime/startup state, for example a real sandbox policy passed to `system_bwrap_warning`. | The code uses a hard-coded sandbox policy or emits a generic startup warning without a source. |
| `guardianWarning` | The warning comes from a real guardian / auto-approval review runtime update. | The test only pushes `AppServerHookNotification::GuardianWarning` into the queue. |
| `model/verification` | A real provider/runtime metadata path populates `ResponseMetadata.model_verifications`, and app-server only maps that explicit metadata. | The source is a scripted responder, `actual_model`, or a test-only fixture with no provider parser coverage. |
| `model/rerouted` | A real provider/runtime event supplies both the selected model and a trustworthy reroute reason. | The only evidence is requested/actual model mismatch. |

Patch-style code is disallowed:

- Do not set `RuntimeBridgeFeatures.model_reroutes = true` or `model_verifications = true` just to make capability tests pass.
- Do not advertise `warnings` or `guardian_warnings` because the delivery queue can serialize the notification.
- Do not add a client-side tolerance test that accepts missing events as equivalent to supported events.
- Do not convert a fake test bridge or manual notification enqueue into "producer" proof.

---

## 2. Task 1 - Lock Truthful R6 Readiness

Files:

- `crates/dasclaw_app_server/src/lib.rs`
- `crates/dasclaw_app_server/src/app_services.rs`

Steps:

- [ ] Keep or add the regression test `real_r6_services_advertise_only_wired_notification_producers` in `crates/dasclaw_app_server/src/lib.rs`.
- [ ] Assert exact event lists, not partial containment, so accidental over-advertising fails.
- [ ] Keep `warning`, `guardianWarning`, `model/rerouted`, and `model/verification` out of real-service `implemented_events` until later tasks provide real sources.
- [ ] Do not make the test pass by weakening protocol expectations or by hiding unavailable events on the client side.

Expected test shape:

```rust
#[test]
fn real_r6_services_advertise_only_wired_notification_producers() {
    let server = initialized_server_for_test();
    let profile = server.protocol_profile();
    let r6 = profile.capabilities.r6;

    assert!(r6.hooks);
    assert!(r6.config_warnings);
    assert!(r6.deprecation_notices);
    assert!(!r6.warnings);
    assert!(!r6.guardian_warnings);
    assert!(!r6.model_reroutes);
    assert!(!r6.model_verifications);

    let implemented = r6.implemented_events();
    assert_eq!(
        implemented.hooks,
        vec!["hook/started".to_string(), "hook/completed".to_string()]
    );
    assert_eq!(
        implemented.warnings,
        vec![
            "configWarning".to_string(),
            "deprecationNotice".to_string(),
        ]
    );
    assert!(implemented.model_provider_events.is_empty());
}
```

Verification:

```bash
cargo nextest run -p dasclaw_app_server -E 'test(real_r6_services_advertise_only_wired_notification_producers)'
```

---

## 3. Task 2 - Separate Delivery From Producer Readiness

Files:

- `crates/dasclaw_app_server/src/app_services.rs`
- `crates/dasclaw_app_server/src/lib.rs`
- `crates/dasclaw_app_server_protocol/src/lib.rs`

Steps:

- [ ] Audit every place that writes `AppServerR6Availability`.
- [ ] Keep delivery-only support out of availability booleans.
- [ ] Add helper methods with producer-oriented names so future edits cannot confuse serialization with production.
- [ ] Add tests for both real services and fake/scripted services.

Implementation pattern:

```rust
impl RuntimeBridgeFeatures {
    fn real_model_r6_availability(self) -> (bool, bool) {
        (
            self.model_reroutes,
            self.model_verifications,
        )
    }
}
```

Use the helper only where the runtime bridge has a real producer contract:

```rust
let (model_reroutes, model_verifications) =
    self.runtime_features.real_model_r6_availability();
availability.r6.model_reroutes = model_reroutes;
availability.r6.model_verifications = model_verifications;
```

Regression tests:

- [ ] A real app-server instance does not advertise model events when the real bridge has no source.
- [ ] A scripted bridge can still test delivery, but the test name must say `delivery` or `scripted`, not `producer`.
- [ ] A manually queued hook/warning notification does not flip service readiness.

Verification:

```bash
cargo nextest run -p dasclaw_app_server -E 'test(real_r6_services_advertise_only_wired_notification_producers) | test(runtime_model_reroute_update_emits_notification) | test(warning_service_events_drain_to_json_rpc_notifications)'
```

---

## 4. Task 3 - Wire Remaining Real Hook Lifecycle Producers

Files:

- `crates/dasclaw_app_server/src/lib.rs`
- `crates/dasclaw_app_server/src/hook_service.rs`
- `crates/dasclaw_hooks/src/hook.rs` only if a missing event field makes app-server wiring impossible

Current real path:

- `RuntimeClientToolExecutor::execute` calls `run_before_tool_call_hook`.
- `run_before_tool_call_hook` creates `dasclaw_hooks::HookEvent::ToolCall`.
- `HookRegistry` observers feed `AppServerHookService`, which maps `beforeToolCall` to `HookEventName::PreToolUse`.

Remaining real lifecycle points to wire:

| Hook point | Real app-server source | Event |
|---|---|---|
| `BeforeInbound` | `turn_start` after thread and turn are known, before runtime bridge start | `HookEvent::Inbound` |
| `BeforeOutbound` | runtime completed output before `turn/completed` notification | `HookEvent::Outbound` |
| `TransformResponse` | only if app-server actually applies the transformed response | `HookEvent::ResponseTransform` |
| `OnSessionStart` | thread/session creation path | `HookEvent::SessionStart` |
| `OnSessionEnd` | thread/session shutdown or app-server shutdown path | `HookEvent::SessionEnd` |

Steps:

- [ ] Add a small app-server helper that runs a hook event and converts `HookError::Rejected` into the same fail-safe path used by tool-call hooks.
- [ ] Call it from `turn_start` for `HookEvent::Inbound` before runtime execution starts.
- [ ] Call it on completed runtime output for `HookEvent::Outbound` before emitting `turn/completed`.
- [ ] Wire `SessionStart` and `SessionEnd` to real lifecycle code only if the app-server can identify a stable session/thread id at those points.
- [ ] Do not claim `TransformResponse` support unless app-server uses the transformed value returned by the hook. If the current `dasclaw_hooks` API cannot return transformed text for app-server safely, keep that specific lifecycle point disabled/documented.
- [ ] Keep `hook/started` and `hook/completed` advertised only for lifecycle points that are actually reachable.

Suggested helper shape:

```rust
async fn run_app_server_hook_event(
    registry: Option<&Arc<dasclaw_hooks::HookRegistry>>,
    event: dasclaw_hooks::HookEvent,
) -> Result<(), AppServerError> {
    let Some(registry) = registry else {
        return Ok(());
    };

    registry.run(&event).await.map(|_| ()).map_err(|error| match error {
        dasclaw_hooks::HookError::Rejected { reason } => {
            AppServerError::invalid_params(reason)
        }
        other => AppServerError::internal(other.to_string()),
    })
}
```

Test requirements:

- [ ] Register a real `HookRegistry` observer through `AppServerHookService`, not by pushing queue entries.
- [ ] Run a normal app-server `turn_start`.
- [ ] Drain JSON-RPC notifications and assert `hook/started` plus `hook/completed` for the expected event name.
- [ ] Add a rejection test for `BeforeInbound` to prove fail-safe behavior.
- [ ] Add an outbound test that fails if output is emitted without the real hook observer run.

Required test names:

- `turn_start_runs_inbound_hook_and_emits_real_hook_notifications`
- `inbound_hook_rejection_blocks_turn_start_fail_safe`
- `completed_turn_runs_outbound_hook_before_turn_completed`

Verification:

```bash
cargo nextest run -p dasclaw_app_server -E 'test(turn_start_runs_inbound_hook) | test(inbound_hook_rejection) | test(completed_turn_runs_outbound_hook) | test(runtime_client_tool_hook_observer_emits_started_and_completed)'
cargo nextest run -p dasclaw_hooks
```

---

## 5. Task 4 - Add Real Generic Warning Producer From Actual Sandbox Policy

Files:

- `crates/dasclaw_app_server/src/lib.rs`
- `crates/dasclaw_app_server/src/app_services.rs`
- Optional new file: `crates/dasclaw_app_server/src/startup_warning_service.rs`
- `crates/dasclaw_sandboxing/src/bwrap.rs` only if testability requires a public helper or injectable detector

Source rule:

Use `dasclaw_sandboxing::system_bwrap_warning(&SandboxPolicy)` only with the actual resolved sandbox policy for the app-server context or turn. Never create a warning by passing a hard-coded `SandboxPolicy::ReadOnly { network_access: false }`.

Steps:

- [ ] Identify where app-server resolves `TurnStartParams.sandbox_policy` into the runtime sandbox context.
- [ ] Add a helper that accepts the resolved policy and returns `WarningNotification`.
- [ ] Emit `warning` with `thread_id: Some(thread_id)` for turn-specific sandbox warnings.
- [ ] If there is no app-server startup-wide sandbox policy, do not emit a startup-wide warning.
- [ ] Advertise `r6.warnings = true` only after the real resolved-policy path is wired and tested.

Suggested helper:

```rust
fn warning_from_resolved_sandbox_policy(
    thread_id: &str,
    sandbox_policy: &dasclaw_sandboxing::SandboxPolicy,
) -> Option<WarningNotification> {
    dasclaw_sandboxing::system_bwrap_warning(sandbox_policy).map(|message| {
        WarningNotification {
            thread_id: Some(thread_id.to_string()),
            message,
        }
    })
}
```

Test requirements:

- [ ] A test with a real resolved policy emits `warning` when `system_bwrap_warning` returns a warning.
- [ ] A test with `DangerFullAccess` or `ExternalSandbox` emits no warning.
- [ ] A test proves no warning is emitted when `TurnStartParams.sandbox_policy` is absent and there is no resolved startup policy.
- [ ] A readiness test proves `warnings` remains false until this producer is actually wired.

If `system_bwrap_warning` is difficult to test deterministically because it probes the host PATH, add a narrow test seam in `dasclaw_sandboxing` instead of stubbing app-server behavior:

```rust
#[cfg(test)]
pub(crate) fn system_bwrap_warning_for_test(
    sandbox_policy: &SandboxPolicy,
    system_bwrap_path: Option<&std::path::Path>,
) -> Option<String> {
    if !should_warn_about_system_bwrap(sandbox_policy) {
        return None;
    }
    system_bwrap_warning_for_path(system_bwrap_path)
}
```

Verification:

```bash
cargo nextest run -p dasclaw_app_server -E 'test(sandbox_warning) | test(real_r6_services_advertise_only_wired_notification_producers)'
cargo nextest run -p dasclaw_sandboxing -E 'test(bwrap_warning)'
```

---

## 6. Task 5 - Add Guardian Warning Producer From Real Auto-Approval Review Updates

Files:

- `crates/dasclaw_app_server/src/lib.rs`
- `crates/dasclaw_app_server/src/app_services.rs`
- `crates/dasclaw_app_server_protocol/src/lib.rs` only if the DTO needs an additional source/status field

Real source:

`RuntimeTurnOutcome::AutoApprovalReviewCompleted` already carries `RuntimeAutoApprovalReviewUpdate`, whose `review` is `GuardianApprovalReview`. Use that runtime update as the source for `guardianWarning`.

Steps:

- [ ] Add a pure helper that maps blocked, denied, rejected, timed-out, or high-risk review completions to `GuardianWarningNotification`.
- [ ] Call the helper in the existing `RuntimeTurnOutcome::AutoApprovalReviewCompleted` handling after emitting `item/autoApprovalReview/completed`.
- [ ] Set `r6.guardian_warnings = true` only when the runtime bridge supports `auto_approval_review` and this helper is wired.
- [ ] Do not emit guardian warnings from test queue injection as producer proof.

Suggested helper:

```rust
fn guardian_warning_from_auto_approval_review(
    thread_id: &str,
    update: &RuntimeAutoApprovalReviewUpdate,
) -> Option<GuardianWarningNotification> {
    let status = update.review.status.as_str();
    let risk = update.review.risk_level.as_deref();
    let should_warn = matches!(
        status,
        "blocked" | "denied" | "rejected" | "timeout" | "timedOut" | "failed"
    ) || matches!(risk, Some("high") | Some("critical"));

    if !should_warn {
        return None;
    }

    let message = update
        .review
        .rationale
        .clone()
        .unwrap_or_else(|| format!("Guardian review {} for {}", status, update.action));

    Some(GuardianWarningNotification {
        thread_id: thread_id.to_string(),
        message,
    })
}
```

Test requirements:

- [ ] A normal runtime update with review status `approved` emits only `item/autoApprovalReview/completed`, not `guardianWarning`.
- [ ] A real `AutoApprovalReviewCompleted` runtime update with status `blocked` emits both completed review and `guardianWarning`.
- [ ] The warning message uses the review rationale when present.
- [ ] Readiness advertises `guardianWarning` only when `auto_approval_review` is supported by the runtime bridge.

Verification:

```bash
cargo nextest run -p dasclaw_app_server -E 'test(auto_approval_review_completed) | test(guardian_warning)'
```

---

## 7. Task 6 - Keep Model Reroute And Verification Honest

Files:

- `crates/dasclaw_core/src/response_types.rs`
- `crates/dasclaw_runtime/src/llm_adapter.rs`
- `crates/dasclaw_llm_provider/src/provider/openai_codex_provider.rs`
- `crates/dasclaw_llm_provider/src/provider/codex_chatgpt.rs`
- `crates/dasclaw_llm_provider/src/provider/claw_code_provider.rs`
- `crates/dasclaw_app_server/src/lib.rs`
- `crates/dasclaw_app_server_protocol/src/lib.rs`

Current state:

- `ResponseMetadata.actual_model` is real and preserved.
- `ResponseMetadata.model_verifications` exists.
- App-server emits `model/verification` only when `metadata.model_verifications` is non-empty.
- Existing tests correctly prevent inferring `model/rerouted` from `actual_model` alone.
- Literal search did not find a real provider parser populating `model_verifications`.

Steps:

- [ ] Keep `model/rerouted` disabled until a real provider/runtime source supplies a trustworthy reroute reason.
- [ ] Keep the negative test `runtime_completed_event_does_not_infer_high_risk_reroute_from_actual_model_metadata`.
- [ ] Keep `model/verification` disabled for real bridges unless a provider parser fixture proves explicit verification metadata reaches `ResponseMetadata.model_verifications`.
- [ ] If a real upstream verification field exists, parse that field in the relevant provider and add a provider parser test. Do not use `actual_model` as substitute evidence.
- [ ] If no real upstream field exists, update the gap matrix to say `model/verification` has a typed metadata path and delivery, but no real provider producer yet.

Allowed provider parser pattern only when the upstream field is explicit:

```rust
fn parse_model_verifications(response: &serde_json::Value) -> Vec<ResponseModelVerification> {
    response
        .get("model_verifications")
        .and_then(|value| value.as_array())
        .into_iter()
        .flatten()
        .filter_map(|value| match value.as_str()? {
            "trustedAccessForCyber" | "trusted_access_for_cyber" => {
                Some(ResponseModelVerification::TrustedAccessForCyber)
            }
            _ => None,
        })
        .collect()
}
```

Required provider-level test if parser is added:

```rust
#[test]
fn response_completed_preserves_explicit_model_verification_metadata() {
    let events = r#"
data: {"type":"response.completed","response":{"model":"gpt-test","status":"completed","model_verifications":["trustedAccessForCyber"],"usage":{"input_tokens":1,"output_tokens":1}}}

"#;
    let parsed = parse_sse_response(events).expect("parse response");
    assert_eq!(parsed.actual_model.as_deref(), Some("gpt-test"));
    assert_eq!(
        parsed.model_verifications,
        vec![ResponseModelVerification::TrustedAccessForCyber]
    );
}
```

If the provider internal parsed type currently lacks `model_verifications`, extend it and pass it into `ToolCompletionResponse.metadata`:

```rust
ResponseMetadata {
    actual_model: parsed.actual_model,
    model_verifications: parsed.model_verifications,
    ..ResponseMetadata::default()
}
```

Verification:

```bash
cargo nextest run -p dasclaw_llm_provider -E 'test(model_verification) | test(actual_model)'
cargo nextest run -p dasclaw_runtime -E 'test(tool_completion_response_carries_actual_model_metadata)'
cargo nextest run -p dasclaw_app_server -E 'test(runtime_completed_event_emits_model_verification_from_response_metadata) | test(runtime_completed_event_does_not_infer_high_risk_reroute_from_actual_model_metadata)'
```

Completion decision:

- If no provider parser test can be written from real upstream fields, do not mark `model/verification` complete.
- If no real reroute reason source exists, do not mark `model/rerouted` complete.
- In both cases, leaving the capability disabled is the correct fix, not a partial failure.

---

## 8. Task 7 - Update Gap Matrix With Split Status

Files:

- `docs/plans/dasclaw-app-server-codex-protocol-gap-matrix.md`
- `docs/superpowers/plans/2026-06-23-dasclaw-app-server-r6-config-repo-search-owner.md` only if its R6 status summary needs a correction link to this plan

Steps:

- [x] Replace any broad "R6 complete" wording with split status.
- [x] Mark completed only where route, producer, readiness, and tests all pass.
- [x] Keep unfinished items explicit:
  - `model/rerouted`: unfinished unless a real reroute reason producer exists.
  - `model/verification`: unfinished unless a real provider metadata parser exists.
  - non-Linux generic `warning`: unavailable because `system_bwrap_warning` is Linux-only and must not be advertised from a fake source.
  - startup-wide `warning`: unavailable until there is a real startup-wide sandbox policy source.
- [x] Add a note that delivery-only tests are not completion evidence.

Suggested matrix wording after implementation:

```markdown
R6 remaining producer status:
- Completed: HookRegistry `beforeToolCall` / `userPromptSubmit` / `postToolUse` producers, config warning producer, deprecation notice producer, Linux resolved-sandbox generic warning producer, and guardian warning from auto-approval review updates.
- Still not complete: `model/rerouted` until a real provider/runtime reroute reason exists; `model/verification` until a real provider parser populates `ResponseMetadata.model_verifications`.
- Delivery-only: manually queued warning / guardian / model runtime outcomes remain tests of serialization, not producer readiness.
```

Verification:

```bash
rg -n "R6|model/rerouted|model/verification|guardianWarning|warning|hook/started|hook/completed" docs/plans/dasclaw-app-server-codex-protocol-gap-matrix.md
```

---

## 9. Task 8 - Full Verification Pass

Run after all code/doc changes in the implementation branch:

```bash
cargo fmt --all
cargo nextest run -p dasclaw_app_server -E 'test(real_r6_services_advertise_only_wired_notification_producers) | test(runtime_client_tool_hook_observer_emits_started_and_completed) | test(warning_service_events_drain_to_json_rpc_notifications) | test(runtime_completed_event_emits_model_verification_from_response_metadata) | test(runtime_completed_event_does_not_infer_high_risk_reroute_from_actual_model_metadata) | test(auto_approval_review_completed)'
cargo check -p dasclaw_app_server --tests
python3.12 scripts/check_no_panics.py --base origin/xClaw
```

If provider or sandboxing code changes:

```bash
cargo nextest run -p dasclaw_llm_provider -E 'test(model_verification) | test(actual_model)'
cargo nextest run -p dasclaw_sandboxing -E 'test(bwrap_warning)'
cargo check -p dasclaw_llm_provider --tests
cargo check -p dasclaw_sandboxing --tests
```

If hook crate code changes:

```bash
cargo nextest run -p dasclaw_hooks
cargo check -p dasclaw_hooks --tests
```

Final local quality gate:

```bash
cargo clippy --no-deps -p dasclaw_app_server --all-targets -- -D warnings
```

If clippy reports only pre-existing untouched warnings outside this slice, record the exact output in the final report and let CI be the source of truth per project rules.

Verification executed on 2026-06-25:

- `cargo fmt --all --check` passed.
- `cargo check -p dasclaw_app_server --tests && cargo check -p dasclaw_app_server_protocol --tests` passed.
- `cargo nextest run -p dasclaw_app_server -E 'test(turn_start_runs_inbound_hook) | test(inbound_hook_rejection) | test(inbound_hook_modification) | test(completed_turn_runs_outbound_hook) | test(outbound_hook_rejection) | test(outbound_hook_modification) | test(auto_approval_review_completed) | test(guardian_warning) | test(real_r6_services_advertise_only_wired_notification_producers) | test(capabilities_advertise_ready_r6_warning_producers_only) | test(manually_queued_warning_delivery_does_not_flip_producer_readiness) | test(turn_start_warning_conversion_error) | test(converts_workspace_policy_to_codex_policy_explicitly) | test(codex_policy_conversion_rejects_relative_workspace_roots) | test(turn_start_external_sandbox_policy_does_not_emit_generic_warning) | test(warning_service_events_drain_to_json_rpc_notifications)'` passed, with nextest reporting 18 passed and 2 leaky tests.
- `cargo nextest run -p dasclaw_app_server -E 'test(hook) | test(warning) | test(guardian) | test(sandbox)'` passed, with 38 tests passed.
- `cargo clippy --no-deps -p dasclaw_app_server --all-targets -- -D warnings` passed.
- `git diff --check` passed.
- `python3.12 scripts/check_no_panics.py --base origin/xClaw` could not run in this environment because `python3.12` is not installed; `python3` is 3.9.6 and cannot parse the script's `int | None` type syntax.

---

## 10. Implementation Order

Recommended order:

1. Task 1: lock truthful readiness.
2. Task 2: split delivery and producer readiness.
3. Task 3: hook lifecycle producers.
4. Task 4: generic warning from actual sandbox policy.
5. Task 5: guardian warning from real auto-approval review updates.
6. Task 6: model honesty pass. Implement provider parser only if a real upstream field exists; otherwise keep disabled and document.
7. Task 7: update gap matrix.
8. Task 8: run verification.

Stop condition:

- Stop and report partial completion if model reroute or model verification has no real upstream source.
- Do not create a fake producer to make the R6 row fully green.
- The correct output can be "R6 producer slice completed except model events remain intentionally unavailable"; that is better than a patch-style full completion claim.
