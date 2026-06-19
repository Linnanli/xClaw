# Dasclaw App Server Protocol v0

> 状态：更新至 native v2-shaped chat-session protocol consolidation / runtime bridge / sidecar diagnostics / legacy desktop chat adapter removal
> 日期：2026-06-19
> 目标：定义 Electron / open-cowork shell 可消费的第一版 dasclaw app-server method、event、capability schema。

## 0. Design constraints

| Constraint | Decision |
|---|---|
| 不重写 agent loop | app-server protocol 只暴露 lifecycle / host / service contract，不定义 agent loop internals |
| 不重写 `ToolExecutor` | tool execution contract stays in `dasclaw_runtime` |
| 不绑定 Tauri | protocol 不出现 Tauri command/event 类型 |
| 不整体搬 Codex app-server | Codex `app-server-protocol` / `app-server-client` 仅作 request/event/client 形态参考 |
| 优先兼容 Codex app-server v2 | open-cowork 后续客户端基座会消费 Codex app-server 能力，因此 dasclaw app-server 后续 contract 应先对齐 Codex v2 Thread / Turn / Item 原语，再保留必要的 dasclaw runtime 适配 |
| capability matrix 一等公民 | `initialize` 必须返回能力矩阵，GUI 不能猜后端能力 |
| failure is explicit | degraded、failed、version_mismatch、policy_unavailable 等状态必须结构化 |

Dasclaw app-server exposes one native protocol. For the implemented chat-session surface, that native protocol intentionally uses Codex app-server v2-shaped method names, event names, and payload fields where Dasclaw can support the semantics truthfully. The compatibility profile is descriptive metadata for this subset; it is not a second wire layer and does not add aliases.

### 0.1 Codex app-server v2 compatibility finding (2026-06-08)

本轮允许跨项目对账后，按三层验证确认：参考 `codex-cli-main` 的 app-server 实现是可行且推荐的方向，但参考边界应放在 **v2 protocol / generated schema / app-server client / consumer fixtures**，不应整体移植 Codex 的 `MessageProcessor` 或 Codex runtime processor。

| Evidence layer | Checked source | Finding |
|---|---|---|
| Level 1 概念层 | code-review-graph repo registry、`codex app-server protocol` / `rich interface app server` / `open-cowork app-server consumer` 概念定位 | Codex app-server 是 rich client control-plane，公开 Thread / Turn / Item 三层原语和 JSON-RPC contract。 |
| Level 2 结构层 | `codex-rs/app-server-protocol/src/protocol/common.rs`、`protocol/v2.rs`、`app-server-client/src/lib.rs`、`app-server/src/message_processor.rs` | Codex v2 以 `ClientRequest` / `ServerNotification` 类型和 schema export 维护契约；processor 内部仍深度耦合 Codex core、auth、thread manager、MCP、plugins、fs、approval 等产品服务。 |
| Level 3 字面层 | `rg` 精确核对 `thread/start`、`thread/read`、`turn/start`、`turn/interrupt`、`item/agentMessage/delta`、`turn/completed`、`notifications/initialized` | Codex v2 的核心聊天消费面是 `thread/start -> turn/start -> item/* -> turn/completed`，Electron/open-cowork 打包产物也按这些 notification 更新 conversation state。 |

Compatibility implication:

| Area | Current dasclaw v0 | Codex app-server v2 reference | Next direction |
|---|---|---|---|
| Thread creation | `thread/start` + `thread/started` | `thread/start` + `thread/started` | Public chat-session creation uses the v2-shaped native name and nested `thread` response. Legacy `thread/create` is intentionally unsupported on the public surface. |
| Turn cancellation | `turn/interrupt` returns `{}` and terminal state is represented through `turn/completed.turn.status="interrupted"` | `turn/interrupt` returns `{}` | Public cancellation uses the v2-shaped interrupt request. Legacy `turn/cancel` is intentionally unsupported on the public surface. |
| Streaming text | `item/agentMessage/delta` with `item/started` / `item/completed` | `item/agentMessage/delta` with `item/started` / `item/completed` | Item-level events are the native text stream. Legacy `turn/delta` is historical only. |
| Terminal event | `turn/completed` with nested `Turn.status` and optional `Turn.error` | `turn/completed` with `Turn.status` and optional error | Success, failure, and interrupt all converge on `turn/completed { threadId, turn }`. Legacy `turn/failed` / `turn/cancelled` are historical only. |
| Initialize handshake | `initialize` request returns descriptive `codex_app_server_v2_chat_session_subset` metadata and emits `notifications/initialized` | `initialize` request followed by `notifications/initialized` client notification | The profile describes the native chat-session subset; it does not enable a separate compatibility wire layer. |
| Schema discipline | `protocol/schema` custom discovery | generated TypeScript/JSON Schema from typed protocol | Future compatibility work should add generated fixture or golden schema tests instead of inventing UI-only shape. |

Non-goals preserved: DLP, jobs, skills, MCP, sandbox, approval orchestration, and tool-call UI contract are still not migrated in this stage.

## 1. Transport v0

Phase 1 推荐先采用 **stdio JSON-RPC**：

| Option | Phase 1 status | Reason |
|---|---|---|
| stdio JSON-RPC | default | 最适合 Electron sidecar PoC，启动/退出和日志收集简单 |
| Unix socket / named pipe | reserved | 适合长期本地 service，但第一批会增加 supervisor 复杂度 |
| local HTTP/WebSocket | reserved | 适合 browser/debug tooling，但需要额外 auth / port / CORS 治理 |

Phase 1 binary probes:

| Probe | Output | Intended consumer |
|---|---|---|
| `--version-json` | `ServerInfo` | installer / supervisor version discovery |
| `--health-once` | lightweight pre-initialize `HealthCheckResponse` with service details | process liveness checks |
| `--capabilities-once` | `CapabilitiesListResponse` | GUI feature gating before starting stdio |
| `--schema-once` | `ProtocolSchemaResponse` | GUI protocol bootstrap without a long-lived session |
| `--self-check` | initialize + lifecycle + health + capabilities + schema + in-memory session smoke report | local smoke probe for diagnostics and supervisor helper validation |

Desktop lifecycle can opt into a long-lived sidecar supervisor with `DASCLAW_APP_SERVER_SUPERVISOR=1`. In that mode, the legacy desktop engine starts a stdio app-server sidecar, performs `initialize` and recurring `health/check`, records restart/backoff status plus a connection generation, and sends `shutdown` when the legacy Agent loop exits. Diagnostics expose the supervisor state, restart count, notification count, runtime health, and connection generation; chat execution is intentionally not routed through this sidecar transport.

The former desktop `send_chat_message` app-server probes/dispatch gates were removed from the plan and implementation. `DASCLAW_APP_SERVER_CHAT_INPROCESS_PROBE`, `DASCLAW_APP_SERVER_CHAT_SIDECAR_PROBE`, and `DASCLAW_APP_SERVER_CHAT_SIDECAR_DISPATCH` are not compatibility surfaces. They coupled the app-server protocol to the legacy `chat-stream` / `VercelUIStream` contract and risked making old desktop-client behavior the source of truth. Future open-cowork integration should consume the app-server protocol directly.

For the future open-cowork client path, protocol confidence should come from app-server integration tests rather than legacy UI wiring. Because open-cowork already expects Codex app-server style capabilities, new dasclaw client-facing behavior should be validated against a Codex v2 compatibility matrix first. New behavior should first be covered at the protocol/client boundary:

| Test family | Required coverage |
|---|---|
| Session/turn lifecycle | native `initialize`, `thread/start`, `thread/list`, `thread/read`, `thread/turns/list`, `turn/start`, `turn/interrupt`, `turn/read`, `shutdown`; legacy smoke aliases are not part of the public chat-session contract |
| Streaming | response-time notifications plus delayed post-response item-level deltas and nested terminal turn notifications; coverage includes `notifications/initialized`, `thread/started`, `item/started`, `item/agentMessage/delta`, `item/completed`, `turn/completed`, bounded notification queues, and overflow signaling |
| Runtime bridge | fake/test runtime bridge emits completed, failed, cancelled, and multi-delta turns |
| Transport | in-process and line-delimited stdio clients, unexpected response ids, JSON-RPC errors, protocol errors, timeouts |
| Supervisor boundary | lifecycle restart/backoff covers sidecar health; no desktop chat command replay or UI-stream bridge is part of the compatibility surface |

Current coverage note: the direct app-server JSON-RPC integration slice now covers multi-delta completion, runtime failure, pending-turn interruption, `notifications/initialized`, item-level streaming, and queue overflow signaling. The line-delimited client also covers response-first nested `turn/completed` typed notifications for completed, failed, and interrupted turns. Remaining protocol-first work should expand truthful Codex app-server capability coverage rather than deepening historical smoke names.

JSON-RPC envelope 使用稳定字段：

```json
{
  "jsonrpc": "2.0",
  "id": "req_01",
  "method": "initialize",
  "params": {}
}
```

Notification 没有 `id`：

```json
{
  "jsonrpc": "2.0",
  "method": "lifecycle/changed",
  "params": {}
}
```

## 2. Versioning

```ts
type ProtocolVersion = {
  major: 0;
  minor: number;
  patch: number;
};

type ClientInfo = {
  name: "open-cowork" | "desktop-client" | string;
  version: string;
  transport: "stdio" | "unix_socket" | "named_pipe" | "http_ws";
};

type ServerInfo = {
  name: "dasclaw_app_server";
  version: string;
  protocolVersion: ProtocolVersion;
};
```

Compatibility rule for v0:

| Case | Result |
|---|---|
| same `major` | accept, capability matrix decides feature availability |
| unknown newer `major` | reject with `VERSION_MISMATCH` |
| missing client info | accept only in dev mode; production should reject |

## 3. Lifecycle model

```ts
type LifecycleState =
  | "starting"
  | "initializing"
  | "ready"
  | "running"
  | "awaiting_approval"
  | "degraded"
  | "failed"
  | "restarting"
  | "stopping"
  | "stopped";

type LifecycleReason =
  | "process_started"
  | "initialize_requested"
  | "runtime_ready"
  | "request_in_progress"
  | "approval_pending"
  | "policy_degraded"
  | "dlp_degraded"
  | "provider_degraded"
  | "version_mismatch"
  | "shutdown_requested"
  | "internal_error";

type LifecycleSnapshot = {
  state: LifecycleState;
  reason: LifecycleReason;
  message?: string;
  since: string;
  degradedServices?: ServiceHealth[];
};
```

Lifecycle guardrail:

| State | Meaning |
|---|---|
| `ready` | app-server can accept supported methods |
| `running` | at least one host operation is active |
| `awaiting_approval` | runtime or host is blocked on GUI/user decision |
| `degraded` | app-server is alive but one or more services are fail-safe degraded |
| `failed` | app-server cannot serve supported methods until restart or reinitialize |

## 4. Capability schema

```ts
type CapabilityStatus = "implemented" | "declared" | "disabled" | "unavailable";

type Capability = {
  id: string;
  status: CapabilityStatus;
  version: string;
  methods: string[];
  events: string[];
  reason?: string;
};

type CapabilityMatrix = {
  protocol: Capability;
  lifecycle: Capability;
  health: Capability;
  session: Capability;
  approval: Capability;
  dlpPolicy: Capability;
  modelProvider: Capability;
  tools: Capability;
  jobs: Capability;
  skills: Capability;
  mcp: Capability;
  sandbox: Capability;
  logs: Capability;
};
```

Phase 1 default matrix:

| Capability | Status | Methods | Events |
|---|---|---|---|
| `protocol` | implemented | `initialize`、`protocol/schema` | `notifications/initialized` |
| `lifecycle` | implemented | `lifecycle/status`、`shutdown` | `lifecycle/changed` |
| `health` | implemented | `health/check`、`capabilities/list` | `health/changed`、`capabilities/changed` |
| `logs` | declared | none | `log/entry` |
| `session` | implemented | `thread/start`、`thread/list`、`thread/read`、`thread/turns/list`、`turn/start`、`turn/interrupt`、`turn/read` | `thread/started`、`turn/started`、`turn/completed`、`item/started`、`item/agentMessage/delta`、`item/reasoning/summaryTextDelta`、`item/reasoning/summaryPartAdded`、`item/reasoning/textDelta`、`item/completed`、`error` |
| `approval` | declared | none | none |
| `dlpPolicy` | declared | none | none |
| `modelProvider` | declared | none | none |
| `tools` | declared | none | none |
| `jobs` | declared | none | none |
| `skills` | declared | none | none |
| `mcp` | declared | none | none |
| `sandbox` | declared | none | none |

Rule: `implemented` means every advertised method has a handler. A future capability may be `declared` to reserve protocol shape, but the GUI must not call missing methods.

Phase 1.6 status sync:

- `session` is now advertised as `implemented`, because thread/turn routes, notifications, and the runtime bridge boundary are live in the app-server crate.
- `runtime` is still reported as `degraded` by health when the default host is using `NoopRuntimeBridge`; a real runtime adapter must be injected to execute turns.

## 5. Methods v0

### 5.1 `initialize`

```ts
type InitializeParams = {
  client: ClientInfo;
  protocolVersion: ProtocolVersion;
  workspace?: {
    root?: string;
    trust?: "trusted" | "untrusted" | "unknown";
  };
  requestedCapabilities?: string[];
};

type InitializeResponse = {
  server: ServerInfo;
  lifecycle: LifecycleSnapshot;
  capabilities: CapabilityMatrix;
  compatibilityProfiles?: CompatibilityProfile[];
  unavailableRequestedCapabilities?: string[];
};

type CompatibilityProfile = {
  id: "codex_app_server_v2_chat_session_subset" | string;
  version: string;
  scope: "chat_session_subset";
  description: string;
  methods: string[];
  events: string[];
  aliases: { legacy: string; compatible: string; kind: "alias" | "deprecated_helper" | "legacy_smoke_surface" }[];
  capabilityOptOuts?: { capability: string; reason: "phase_1_chat_session_subset" | string }[];
  eventQueue: NotificationQueuePolicy;
};

type NotificationQueuePolicy = {
  maxPendingNotifications: 256;
  overflow: "lag_disconnect";
};
```

Responsibilities:

| Owner | Responsibility |
|---|---|
| client | send version, transport, requested capability list |
| app-server | validate compatibility, initialize local services, return capability matrix |
| runtime | not invoked for Phase 1 initialize |

Capability negotiation rule:

| Case | Behavior |
|---|---|
| requested capability is `implemented` | omit from `unavailableRequestedCapabilities` |
| requested capability is `declared` / `disabled` / `unavailable` | include the id in `unavailableRequestedCapabilities` |
| requested capability is unknown | include the id in `unavailableRequestedCapabilities` |
| duplicate requested id | include at most once, preserving first-seen order |

### 5.2 `health/check`

### 5.2 `protocol/schema`

```ts
type MethodSchema = {
  method: string;
  capability: string;
  paramsType?: string;
  resultType: string;
  requiresInitialize: boolean;
};

type EventSchema = {
  event: string;
  capability: string;
  payloadType: string;
};

type ProtocolSchemaResponse = {
  protocolVersion: ProtocolVersion;
  methods: MethodSchema[];
  events: EventSchema[];
  capabilities: CapabilityMatrix;
  compatibilityProfiles?: CompatibilityProfile[];
};
```

Phase 1 rule:

| Field | Meaning |
|---|---|
| `methods` | currently routable JSON-RPC methods, not future reservations |
| `events` | event payload types the GUI may subscribe to or parse |
| `capabilities` | same shape as `capabilities/list`, included so GUI can bootstrap from one call |
| `requiresInitialize` | whether a method is callable before `initialize`; all Phase 1 lifecycle/probe methods are currently safe before initialize |

Runtime guard rule:

| Method class | Guard |
|---|---|
| lifecycle/probe methods | callable before `initialize` |
| session methods with `requiresInitialize=true` | return `NOT_INITIALIZED` with `retryable=true` until `initialize` reaches `ready` |
| session methods after `shutdown` | return stopped lifecycle error with `retryable=false`; the process must be restarted |

Schema consistency rule:

| Rule | Requirement |
|---|---|
| implemented capability methods | every method advertised by an `implemented` capability must appear in `protocol/schema.methods` |
| method uniqueness | `protocol/schema.methods[].method` values must be unique |
| event uniqueness | `protocol/schema.events[].event` values must be unique |
| app-server router | every method in `protocol/schema.methods` must be routable by app-server, even when the handler is still a skeleton |
| declared capabilities | may reserve future events/types, but must not advertise unroutable methods as implemented |

### 5.3 `health/check`

```ts
type HealthCheckParams = {
  includeDetails?: boolean;
};

type ServiceHealth = {
  service:
    | "protocol"
    | "lifecycle"
    | "session"
    | "runtime"
    | "dlp_policy"
    | "model_provider"
    | "tools"
    | "sandbox"
    | "jobs"
    | "skills"
    | "mcp"
    | "logs";
  status: "ready" | "degraded" | "disabled" | "unavailable";
  failSafe: boolean;
  message?: string;
};

type HealthCheckResponse = {
  ok: boolean;
  lifecycle: LifecycleSnapshot;
  services: ServiceHealth[]; // empty when includeDetails is false
};
```

Phase 1 health rule:

| Service | Expected status |
|---|---|
| protocol | `ready` |
| lifecycle | `ready` |
| session | `ready`; thread/turn host and runtime bridge boundary are available |
| logs | `disabled` until a real app-server log source is exposed |
| runtime | `degraded` when the default host is still using `NoopRuntimeBridge`; `ready` once a real runtime adapter is injected |
| dlp_policy | `unavailable` or `degraded`; must be fail-safe when enforcement is enabled |
| jobs/skills/mcp/sandbox | `disabled` or `declared` until later slices |

### 5.4 `capabilities/list`

```ts
type CapabilitiesListParams = {};

type CapabilitiesListResponse = {
  capabilities: CapabilityMatrix;
};
```

### 5.5 `lifecycle/status`

```ts
type LifecycleStatusParams = {};

type LifecycleStatusResponse = {
  lifecycle: LifecycleSnapshot;
};
```

### 5.6 `shutdown`

```ts
type ShutdownParams = {
  reason?: "client_exit" | "restart" | "user_requested" | "test";
  timeoutMs?: number;
};

type ShutdownResponse = {
  accepted: boolean;
  lifecycle: LifecycleSnapshot;
};
```

Shutdown rule:

| Situation | Behavior |
|---|---|
| no running operation | transition `stopping -> stopped` |
| running operation | request cancellation first; Phase 1 may reject with `OPERATION_IN_PROGRESS` |
| awaiting approval | default deny/cancel before shutdown in later approval slice |

### 5.7 `thread/start` skeleton

```ts
type ThreadStartParams = {
  cwd?: string;
};

type ThreadStartResponse = {
  thread: {
    id: string;
    status: "inProgress" | "completed" | "failed" | "interrupted";
    createdAt: string;
    updatedAt: string;
  };
  model: string;
  modelProvider: string;
  cwd: string;
};
```

Current behavior: after `initialize`, app-server creates an in-memory thread id and emits `thread/started` with nested `thread`. The response does not expose response-level `threadId`; consumers must read `result.thread.id`. The response also includes truthful `model`, `modelProvider`, and `cwd` metadata derived from selected model-provider state. Before `initialize`, it returns `NOT_INITIALIZED` with `retryable=true`; if no model provider has been selected, it returns a fail-safe error instead of inventing provider metadata.

### 5.8 `thread/list` skeleton

```ts
type ThreadSummary = {
  id: string;
  status: "inProgress" | "completed" | "failed" | "interrupted";
  title?: string;
  workspaceRoot?: string;
};

type ThreadListResponse = {
  data: ThreadSummary[];
  nextCursor?: string;
  backwardsCursor?: string;
};
```

Phase 1.5 behavior: after `initialize`, app-server returns in-memory thread summaries. Before `initialize`, it returns `NOT_INITIALIZED` with `retryable=true`. Thread history and persistence are not implemented.

### 5.9 `thread/read` skeleton

```ts
type ThreadReadParams = {
  threadId: string;
};

type ThreadReadResponse = {
  thread: ThreadSummary;
};
```

Phase 1.5 behavior: after `initialize`, app-server returns the in-memory thread summary for a known `threadId`. Unknown threads return `INVALID_PARAMS`. Message/event history is not implemented.

### 5.10 `turn/start` skeleton

```ts
type TurnStartParams = {
  threadId: string;
  // This slice accepts text input only; rich input items, attachments,
  // approvals, tool calls, MCP, skills, DLP, jobs, and sandbox remain out of scope.
  input: Array<{
    type: "text";
    text: string;
    textElements?: unknown[];
  }>;
};

type TurnStartResponse = {
  turn: {
    id: string;
    threadId: string;
    status: "inProgress" | "completed" | "failed" | "interrupted";
    items: ThreadItem[];
    error?: { message: string };
  };
};
```

Current behavior: app-server first enforces `initialize` and validates that `threadId` exists. If both pass, it allocates an in-progress turn, invokes `RuntimeBridge::start_turn`, emits `turn/started`, and returns a nested `turn` object. The response does not expose response-level `turnId`; consumers must read `result.turn.id`. Streamed item notifications and the terminal `turn/completed` notification are delivered asynchronously as runtime updates arrive. The stdio loop drains same-round-trip notifications before the matching response and continues draining runtime notifications after the response while the transport remains alive or is in its EOF/idle drain window.

### 5.11 `turn/interrupt` skeleton

```ts
type TurnInterruptParams = {
  threadId: string;
  turnId: string;
};

type TurnInterruptResponse = {};
```

Current behavior: app-server first enforces `initialize`, validates that `threadId` exists, drains pending runtime updates, and routes interruption through `RuntimeBridge::cancel_turn`. Known non-terminal turns are marked `interrupted` and the terminal notification stream remains explicit through `item/completed` and `turn/completed`; unknown turns return `INVALID_PARAMS`, and already terminal turns are rejected as invalid requests.

### 5.12 `thread/turns/list` skeleton

```ts
type TurnSummary = {
  threadId: string;
  id: string;
  status: "inProgress" | "completed" | "failed" | "interrupted";
  output?: string;
  error?: string;
};

type ThreadTurnsListParams = {
  threadId: string;
};

type ThreadTurnsListResponse = {
  data: TurnSummary[];
  nextCursor?: string;
  backwardsCursor?: string;
};
```

Current behavior: after `initialize`, app-server validates `threadId`, drains pending runtime updates, and returns turn summaries for that thread. Unknown threads return `INVALID_PARAMS`. Prompt history is not exposed; completed/failed/interrupted turns may include items or error details populated by the runtime bridge.

### 5.13 `turn/read` skeleton

```ts
type TurnReadParams = {
  threadId: string;
  turnId: string;
};

type TurnReadResponse = {
  turn: TurnSummary;
};
```

Current behavior: after `initialize`, app-server validates `threadId`, drains pending runtime updates, and returns one nested turn summary. Unknown turns return `INVALID_PARAMS`. Runtime execution output is exposed through turn items and optional error details; raw prompt history is not returned.

## 6. Events v0

### 6.1 `notifications/initialized`

```ts
type NotificationsInitializedEvent = {
  lifecycle: LifecycleSnapshot;
  compatibilityProfiles: CompatibilityProfile[];
  unavailableRequestedCapabilities?: string[];
  eventQueue: NotificationQueuePolicy;
};
```

Codex v2 compatibility behavior: emitted after `capabilities/changed` when the client requests `codex_app_server_v2_chat_session_subset`. The profile advertises the current chat-session subset, explicit Phase 1 opt-outs, and the bounded notification queue policy. On queue overflow, the server clears pending notifications and emits a single `error` notification with `NOTIFICATION_QUEUE_OVERFLOW` and `retryable=true`; stdio clients should treat this as a lag-disconnect signal and reconnect.

### 6.2 `lifecycle/changed`

```ts
type LifecycleChangedEvent = {
  lifecycle: LifecycleSnapshot;
  previousState?: LifecycleState;
};
```

### 6.3 `health/changed`

```ts
type HealthChangedEvent = {
  ok: boolean;
  services: ServiceHealth[];
};
```

### 6.4 `capabilities/changed`

```ts
type CapabilitiesChangedEvent = {
  capabilities: CapabilityMatrix;
  reason: "initialize" | "config_changed" | "service_degraded" | "service_recovered";
};
```

### 6.5 `log/entry`

```ts
type LogLevel = "trace" | "debug" | "info" | "warn" | "error";

type LogEntryEvent = {
  level: LogLevel;
  target: string;
  message: string;
  time: string;
  fields?: Record<string, string | number | boolean>;
};
```

Log rule: no raw secrets, no raw user prompt, no raw policy payload.

### 6.6 Session skeleton events

```ts
type ThreadStartedEvent = {
  thread: {
    id: string;
    status: "inProgress" | "completed" | "failed" | "interrupted";
  };
};

type TurnStartedEvent = {
  threadId: string;
  turn: {
    id: string;
    threadId: string;
    status: "inProgress" | "completed" | "failed" | "interrupted";
    items: ThreadItem[];
  };
};

type ItemAgentMessageDeltaEvent = {
  threadId: string;
  turnId: string;
  itemId: string;
  delta: string;
};

type ItemCompletedEvent = {
  threadId: string;
  turnId: string;
  item: ThreadItem;
};

type TurnCompletedEvent = {
  threadId: string;
  turn: {
    id: string;
    threadId: string;
    status: "completed" | "failed" | "interrupted";
    items: ThreadItem[];
    error?: { message: string };
  };
};
```

Current behavior: app-server emits the v2-shaped native chat-session events `thread/started`, `turn/started`, `item/started`, item delta events, `item/completed`, and `turn/completed` with nested `turn`. Completed, failed, and interrupted turns are distinguished by `turn.status` and optional `turn.error`. Historical smoke-surface event names (`thread/created`, `turn/delta`, `turn/failed`, `turn/cancelled`) are intentionally unsupported in the current public contract. These events reflect the app-server host boundary; they still do not imply DLP, approval, jobs, MCP, or sandbox migration.

## 7. Error schema

```ts
type ErrorCode =
  | "VERSION_MISMATCH"
  | "UNKNOWN_METHOD"
  | "CAPABILITY_UNAVAILABLE"
  | "INVALID_PARAMS"
  | "NOT_INITIALIZED"
  | "OPERATION_IN_PROGRESS"
  | "SERVICE_DEGRADED"
  | "NOTIFICATION_QUEUE_OVERFLOW"
  | "INTERNAL_ERROR";

type ErrorData = {
  code: ErrorCode;
  message: string;
  lifecycle?: LifecycleSnapshot;
  capability?: string;
  retryable: boolean;
};
```

Recommended mapping:

| Error | Retryable | Notes |
|---|---|---|
| `VERSION_MISMATCH` | false | client must update/downgrade |
| `CAPABILITY_UNAVAILABLE` | false | GUI should hide/disable feature |
| `NOT_INITIALIZED` | true | call `initialize` first |
| `SERVICE_DEGRADED` | maybe | depends on service fail-safe status |
| `OPERATION_IN_PROGRESS` | true | retry after lifecycle changes |

JSON-RPC envelope rule:

| Situation | JSON-RPC code | Response behavior |
|---|---|---|
| malformed JSON | `-32700` | return parse error without `id` |
| valid JSON but invalid request shape with `id` | `-32600` | return invalid request error, preserving `id` |
| valid JSON but invalid notification shape without `id` | n/a | return no response |
| JSON-RPC batch array | `-32600` | reject explicitly; Phase 1 stdio accepts one request/notification per line |
| request with `jsonrpc != "2.0"` and `id` | `-32600` | return invalid request error with the same `id` |
| notification with `jsonrpc != "2.0"` and no `id` | n/a | execute no handler and return no response |
| unknown method request with `id` | `-32601` | return method-not-found error |
| unknown method notification without `id` | n/a | return no response |
| method that accepts no params receives non-empty params | `-32602` | return invalid params error when request has `id` |

## 8. Later protocol slots

These are intentionally out of Phase 1 implementation but reserved in the capability matrix:

| Area | Future methods/events |
|---|---|
| session/thread | Native v2-shaped `thread/start`、`thread/resume`、`thread/read`、`thread/turns/list` |
| approval | `approval/respond`、`approval/requested`、`approval/resolved` |
| chat stream | `thread/started`、`item/started`、`item/agentMessage/delta`、`item/completed`、`turn/completed` |
| tools | Codex-compatible `item/commandExecution/*` / tool-call item events when tool UI migration starts |
| DLP/policy | `policy/status`、`dlp/scan`、`policy/changed` |
| jobs/routines | `job/list`、`job/status`、`job/changed` |
| MCP/skills | `mcp/list_tools`、`skills/list`、status events |
| sandbox | `sandbox/status`、`sandbox/smoke` |

## 9. Phase 1 acceptance criteria

| Criterion | Requirement |
|---|---|
| initialize works | returns server info, lifecycle snapshot, capability matrix |
| health works | returns structured service status |
| lifecycle is explicit | `starting` / `initializing` / `ready` / `failed` / `stopped` are representable |
| no Tauri leakage | schema contains no Tauri-specific command/event names |
| no runtime duplication | schema does not redefine `ToolExecutor` or `AgenticLoop` |
| no capability overclaim | only lifecycle/health/protocol are `implemented` in first skeleton; logs is declared until a log source is wired |
