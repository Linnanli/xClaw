# `dasclaw_runtime`

Headless agent runtime shared across all dasclaw hosts (ironclaw, admin-backend,
claw-code, …). Contains the **application-layer** vocabulary that lets a host
spin up an LLM-driven agent in four lines without pulling in Tauri, a database,
channels, or any desktop infrastructure.

ADR reference: [ADR-153 §3](../../docs/plans/architecture-refactor/adr-153-headless-agent-framework.md)
(headless agent framework) and §4.4 (CLI proof-point).

## At a glance

```rust
use dasclaw_runtime::{Agent, AgentResponder};

let agent = Agent::builder()
    .responder(my_responder)              // anything that knows how to call an LLM
    .system_prompt("You are a helpful assistant.")
    .build()?;
let text = agent.run("Hello!").await?;
```

That is the entire happy path. `my_responder` is anything that implements
[`AgentResponder`](src/agent.rs); the runtime ships
[`LlmProviderResponder`](src/llm_adapter.rs) which adapts any
`dasclaw_llm_provider::LlmProvider` automatically.

## Public surface

Re-exported from `lib.rs` (the only types third-party code should depend on):

| Symbol | Purpose |
|---|---|
| `Agent`, `AgentBuilder`, `AgentConfig` | Headless agent + fluent constructor. |
| `AgentResponder` (trait) | Narrow LLM seam — `async fn respond(ctx) -> RespondOutput`. |
| `ToolExecutor` (trait) | Narrow tool-dispatch seam — `async fn execute(call) -> ToolResult`. |
| `AgentError` | Loop failure modes (`MissingResponder`, `MaxIterations`, `ToolsNotSupported`, …). |
| `LlmProviderResponder` | Ready-made adapter wrapping `dasclaw_llm_provider`. |
| `CompositeToolExecutor`, `CompositeError` | Merge several executors under disjoint tool-name sets. |
| `Tool` | Application-layer tool trait (the in-runtime form). |
| `ToolError` | Application-layer tool error carrying the failing tool's `name`. |
| `JobState`, `StateTransition`, `TokenBudgetExceeded` | Pure job state machine. |
| `JobContextCore` | Per-job runtime context value object. |
| `ToolFeatureFlags`, `SharedFeatureFlags` | Atomic flags consulted by tools. |
| `RateLimiter`, `LimitType`, `RateLimitResult`, `RateLimitError` | Token-bucket rate limiting. |
| `Secret`, `SecretRef`, `DecryptedSecret`, `SecretError`, `SecretsStore` (trait), `CreateSecretParams`, `CredentialLocation`, `CredentialMapping` | Secret-management vocabulary (concrete backends live in hosts). |
| `HttpExchange`, `HttpInterceptor`, … | Recording hooks for replay. |

The two **traits you actually have to implement** to integrate a new host are
`AgentResponder` and (optionally) `ToolExecutor`. Everything else is data.

## Building an agent

`AgentBuilder` (fluent, all setters consume + return `self`):

| Setter | Required? | Notes |
|---|---|---|
| `.responder(impl AgentResponder + 'static)` | **yes** | Or `.responder_arc(Arc<dyn …>)` to share. |
| `.tool_executor(impl ToolExecutor + 'static)` | only when tools are advertised | Or `.tool_executor_arc(...)`. |
| `.tools(Vec<ToolDefinition>)` | only when tool calls are expected | Empty ⇒ text-only mode. |
| `.system_prompt(impl Into<String>)` | no | Prepended to every LLM call. |
| `.model(impl Into<String>)` | no | Per-call model override. |
| `.hooks(HookBundle)` | no | Defaults to `HookBundle::noop()`. |
| `.loop_config(AgenticLoopConfig)` | no | Defaults to 50 iterations + intent nudges on. |
| `.build() -> Result<Agent, AgentError>` | terminal | Fails with `MissingResponder` if `.responder` was not called. |

### Why two narrow traits instead of one fat one

`AgentResponder` is the LLM seam and is a single method.  `ToolExecutor` is
likewise single-method. Splitting along the LLM ↔ tool seam means an adapter
author only implements the side they care about, and the runtime never has to
construct a no-op tool executor for text-only flows.

`dasclaw_core::traits::LlmCompleter` exists too but is **text-only** — it
returns `String`, not `RespondOutput`, so it cannot carry tool calls or finish
reasons. The agent loop fundamentally needs `RespondOutput`, hence the
dedicated `AgentResponder`.

## Failure modes

`Agent::run` returns `Result<String, AgentError>`. The variants:

| Variant | When |
|---|---|
| `MissingResponder` | Builder was missing `.responder(...)`. Caller bug. |
| `MaxIterations(n)` | Loop hit `loop_config.max_iterations` without a final text. |
| `ToolsNotSupported` | Model asked for a tool but no `ToolExecutor` was wired. |
| `LoopFailure(reason)` | `LoopOutcome::Failure(_)`, typically from an egress hook. |
| `Stopped` | External `LoopSignal::Stop` halted the loop. |
| `ApprovalRequested` | **Deprecated** (issue #910). Only surfaced by custom tool-dispatch paths that bypass the approval inbox; the built-in `SequentialDispatcher` emits `AgentEvent::ApprovalNeeded` instead. |
| `ApprovalRejected { tool_name, reason }` | The GUI replied `ApprovalDecision::Reject` for a tool call. |
| `Responder(HostError)` | Underlying responder / hook errored. |

The runtime **does not retry**. If a tool fails, the implementation should
return `ToolResult { is_error: true, ... }`, feeding the failure back to the
model rather than aborting the loop.

## Two-layer `ToolError` model (read once, then forget)

There are *two* `ToolError`s in the codebase, intentionally:

- `dasclaw_tool::ToolError` — tool-impl layer. Returned by a tool's
  `execute()`. Does not carry a tool name (the tool already knows who it is).
- `dasclaw_runtime::ToolError` — dispatcher / application layer. Each variant
  carries the failing tool's `name` plus extra runtime context (e.g.
  `retry_after`).

Conversion is **not** a blanket `From` impl — that would silently fall back to
`"<unknown>"`, which is fail-open and unacceptable. Callers attach the name
explicitly at the boundary via `ToolError::from_tool_impl(name, err)`.

## Composing multiple tool executors

When a host wants tools from two sources (e.g. builtin demo tools **and** MCP
tools), wire them through `CompositeToolExecutor`:

```rust
use std::sync::Arc;
use dasclaw_runtime::{CompositeToolExecutor, ToolExecutor};

let composite = CompositeToolExecutor::new([
    (static_tool_names, Arc::new(static_exec) as Arc<dyn ToolExecutor>),
    (mcp_tool_names,    Arc::new(mcp_exec)    as Arc<dyn ToolExecutor>),
])?; // errors on duplicate tool names
```

The constructor enforces a **disjoint name set** invariant up front, so the
fail-mode is loud and at startup rather than silent and at dispatch time.

## Hooks

`HookBundle` (from `dasclaw_core::hooks`) is passed unchanged into the loop. By
default the runtime uses `HookBundle::noop()`. Hosts can install pre-call /
post-call / pre-tool / post-tool / egress hooks via the standard `HookBundle`
builder API — `dasclaw_runtime` does not wrap it.

## Module map

| Module | Sketch |
|---|---|
| [`agent`](src/agent.rs) | `Agent`, `AgentBuilder`, re-exports `AgentResponder` from core, `ToolExecutor`, `AgentError`, `AgentConfig`. |
| [`composite_executor`](src/composite_executor.rs) | `CompositeToolExecutor` + `CompositeError`. |
| [`context`](src/context/) | Per-job context object & helpers. |
| [`error`](src/error.rs) | App-layer `ToolError`. |
| [`feature_flags`](src/feature_flags.rs) | Atomic shared flags consulted by tools. |
| [`job`](src/job.rs) | Pure `JobState` machine + `TokenBudgetExceeded`. |
| [`job_context`](src/job_context.rs) | `JobContextCore` value object. |
| [`llm_adapter`](src/llm_adapter.rs) | `LlmProviderResponder` — adapt `dasclaw_llm_provider` into `AgentResponder`. |
| [`rate_limit`](src/rate_limit.rs) | Token-bucket rate limiting (`RateLimiter` + types). |
| [`recording`](src/recording.rs) | HTTP exchange record/replay (`HttpExchange`, `HttpInterceptor`). |
| [`secrets`](src/secrets/) | Secret-management vocabulary + `SecretsStore` trait. |
| [`tool`](src/tool.rs) | App-layer `Tool` trait. |

## Stability

`Agent`, `AgentBuilder`, `AgentResponder`, `ToolExecutor`, `AgentError`,
`LlmProviderResponder` and `CompositeToolExecutor` are the **stable public
surface** consumed by `dasclaw_cli` and (in flight) by ironclaw / admin-backend.
Everything else is in-flight and may break before ADR-153 closes — pin to the
exact patch version if you depend on those modules.

## Approval flow (GUI integration, issue #910)

GUIs that want a user-in-the-loop confirmation for risky tool calls
wire three pieces:

1. `.approval_policy(Arc::new(MyPolicy))` on the builder — decides
   whether a given `ToolCall` needs human approval.
2. `Agent::run_streaming(prompt, tx)` — the loop emits an
   `AgentEvent::ApprovalNeeded { request_id, tool_name, … }` whenever
   the policy flags a call, then parks until a decision arrives.
3. `Agent::respond_to_approval(request_id, ApprovalDecision::Approve)`
   (or `Reject { reason }` / `ApproveAlways`) from the UI handler.

`ApprovalDecision` and `ApprovalRequest` are pure data — no channels
or `oneshot::Sender` leak through the public surface — so the same
shape serialises to a TypeScript discriminated union for a Tauri
frontend, an HTTP JSON payload, or a `wasm-bindgen` callback.

End-to-end runnable example (≈170 lines):
[`dasclaw_cli/examples/headless_agent_starter.rs`](../dasclaw_cli/examples/headless_agent_starter.rs).

Run with:

```bash
cargo run -p dasclaw_cli --example headless_agent_starter
```

It shows both the approve path (final text `ok`) and the reject path
(an `AgentError::ApprovalRejected` carrying the policy reason).

## Combine `Agent` with `Session` for replay and persistence

`dasclaw_runtime::Agent` is stateless across turns by design — each
`agent.run(prompt)` call is independent. When a host wants conversation
memory, snapshot/replay, or forks (e.g. branching a chat), wrap the
agent in [`dasclaw_session::Session`](../dasclaw_session/src/lib.rs):

```rust
use std::sync::Arc;
use dasclaw_runtime::Agent;
use dasclaw_session::Session;

let agent = Arc::new(Agent::builder().responder(my_responder).build()?);
let mut session = Session::new(Arc::clone(&agent))
    .with_model("claude-3-5-sonnet")
    .with_workspace_root("/path/to/project");

session.record_prompt("Hello!");
let snapshot = session.snapshot();           // borrow current state
let branch = session.fork(Some("alt".into())); // diverge from this turn
```

Backends are pluggable through the
[`SessionStore`](../dasclaw_session/src/store.rs) trait. The crate
ships [`JsonlSessionStore`](../dasclaw_session/src/jsonl.rs) — an
append-only `.jsonl` file per session for replay across runs. If you
only need an in-process scratchpad, skip the store entirely and use
`Session::snapshot()` directly; the `SessionSnapshot` it returns is
plain `Serialize` data.

The session layer never touches the LLM directly; it only holds
`Arc<Agent>` plus a `SessionSnapshot`. That keeps replay deterministic:
re-creating the same `Agent` config + same snapshot reproduces every
turn.

## See also

- [`dasclaw_cli`](../dasclaw_cli/README.md) — the headless CLI built on this runtime; ADR-153 proof-point.
- [`dasclaw_core`](../dasclaw_core/) — the agentic loop primitives this runtime wraps.
- [`dasclaw_llm_provider`](../dasclaw_llm_provider/) — concrete LLM clients adapted via `LlmProviderResponder`.
- [`dasclaw_mcp`](../dasclaw_mcp/) — MCP transports and `McpToolExecutor` (plugged in by `dasclaw_cli`).
- [`dasclaw_session`](../dasclaw_session/) — `Session` wrapper for replay, snapshots and forks (combine with `Agent` as above).
- ADR-153 — headless agent framework rationale and milestones.
