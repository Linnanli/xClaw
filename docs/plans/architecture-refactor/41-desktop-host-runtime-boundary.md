# 41 — Desktop Client / IronClaw Host / Runtime Boundary

> Issue: [#1054](https://github.com/Linnanli/xClaw/issues/1054)  
> Date: 2026-06-04  
> Scope: clarify when desktop IPC should enter the reusable agent loop, and add execution-level Plan/Fork verification.

## 1. Decision

Desktop chat/tool turns keep the three-layer path:

```text
desktop-client IPC/UI
  -> desktop-client/ironclaw host
  -> dasclaw_runtime::AgenticLoop
```

Platform diagnostics and configuration sync stay outside `AgenticLoop`. They may call shared crates directly, but they should not be forced through the LLM turn loop only to look more uniform.

Plan/Fork controls sit in the middle: they enter the IronClaw host message loop so they share thread resolution, session ownership, and channel routing, but they stop at `SubmissionParser` + `thread_ops` unless the specific command intentionally asks the LLM to revise a plan.

## 2. Boundary Matrix

| Capability | Entry point | Stops at | Rationale |
|---|---|---|---|
| Chat/tool turn | `send_chat_message` / `IncomingMessage` | `dasclaw_runtime::AgenticLoop` | Needs LLM sampling, tool dispatch, hooks, cancellation, and response streaming. |
| Plan mode toggle | `ic_toggle_plan_mode` -> `/plan-mode` | IronClaw host `thread_ops` | Mutates thread state; no LLM sampling needed. |
| Plan approval | `ic_approve_plan` -> `/approve-plan` | IronClaw host `thread_ops` | Consumes a pending plan and switches execution state; no new model turn by itself. |
| Plan revision | `ic_revise_plan` -> `/revise-plan` | IronClaw host, then user-input path | Validates pending plan first; then re-enters normal user-input processing to ask the model for a revised plan. |
| Session fork | `ic_fork_thread` -> `/fork <turn>` | IronClaw host `thread_ops` + `dasclaw_core::Session` | Copies thread history and registers the new thread; no tool dispatch or model call. |
| Sandbox status/smoke | `desktop-client/src/ipc/sandbox.rs` | `dasclaw_sandbox` / platform service | Health check of platform capability, not an agent turn. |
| DLP/admin sync | `admin_sync.rs` / `enterprise_policy_sync.rs` | `SafetyBridge` / `dasclaw_safety` / sync service | Policy vocabulary and admin transport concern; must remain deterministic and fail-safe. |
| Model connection probe | provider/config IPC | provider connectivity layer | Probe validates credentials/config availability; it should not create conversation state. |

## 3. Layer Responsibilities

`desktop-client` owns UI-facing IPC contracts, response shapes, Tauri command registration, desktop startup state, and direct platform probes.

`desktop-client/ironclaw` acts as the host adapter. It owns channel routing, session/thread resolution, `SubmissionParser` handling, host-specific hooks, tool registry composition, desktop safety/context wiring, and the bridge into `dasclaw_runtime`.

`dasclaw_runtime` owns reusable LLM turn orchestration: responder calls, tool dispatch iteration, cancellation, loop outcomes, and hook application that is independent of Tauri or desktop-only services.

`dasclaw_core` owns stable vocabulary and state models that are safe to share across hosts: `Submission`, `Session`, `Thread`, `PendingPlan`, `SessionManager`, and agent loop config/types.

## 4. Migration Guidance

Good extraction candidates:

| Candidate | Current evidence | Direction |
|---|---|---|
| `SubmissionParser` / `Submission` | Already in `crates/dasclaw_core/src/submission.rs` and re-exported by IronClaw | Keep as shared command vocabulary. |
| `Session` / `Thread` / `SessionManager` | Already in `crates/dasclaw_core/src/session.rs` and `session_manager.rs` | Keep shared; add tests at host boundary when commands mutate it. |
| Agent loop contracts | `crates/dasclaw_core/src/agentic_loop.rs` and `dasclaw_runtime::AgenticLoop` | Keep reusable; host provides responder/dispatcher/context. |

Do not migrate in this slice:

| Item | Reason |
|---|---|
| Tauri command registration and frontend response DTOs | Desktop UI contract, not runtime vocabulary. |
| Desktop startup state and Tauri `EngineState` | Tauri lifecycle concern. |
| Admin HTTP, policy sync, and DLP bridge transport | Product/admin integration; deterministic service path is safer. |
| Sandbox smoke/status IPC | Platform readiness check; direct shared-crate call is clearer. |
| Model connection probe IPC | Provider/config health check, not conversation execution. |

## 5. Verification Notes

Three-layer verification performed before writing this boundary:

| Level | Evidence |
|---|---|
| Semantic | `code-review-graph` Python API found Plan/Fork IPC in `desktop-client/src/ipc/plan_mode.rs`, session state in `crates/dasclaw_core/src/session.rs`, and the reusable loop in `crates/dasclaw_core/src/agentic_loop.rs` / `crates/dasclaw_runtime`. |
| Symbol/usage | The execution path is `IncomingMessage` -> `SubmissionParser::parse` -> `Agent::handle_message` -> `process_toggle_plan_mode` / `process_fork_thread` -> `Session` mutation and `SessionManager::register_thread`. |
| Literal | `rg "plan-mode|approve-plan|revise-plan|/fork|fork_thread|toggle_plan_mode"` confirms the concrete IPC, parser, host handler, session, and existing parity tests. |

The new execution-level tests live in `desktop-client/ironclaw/src/agent/agent_loop.rs` and cover the host message loop, not only the IPC enqueue contract from `desktop-client/src/ipc/plan_mode.rs`.
