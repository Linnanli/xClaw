# Dasclaw App Server Protocol v0

> 状态：Phase 0 草案
> 日期：2026-06-07
> 目标：定义 Electron / open-cowork shell 可消费的第一版 dasclaw app-server method、event、capability schema。

## 0. Design constraints

| Constraint | Decision |
|---|---|
| 不重写 agent loop | app-server protocol 只暴露 lifecycle / host / service contract，不定义 agent loop internals |
| 不重写 `ToolExecutor` | tool execution contract stays in `dasclaw_runtime` |
| 不绑定 Tauri | protocol 不出现 Tauri command/event 类型 |
| 不整体搬 Codex app-server | Codex `app-server-protocol` / `app-server-client` 仅作 request/event/client 形态参考 |
| capability matrix 一等公民 | `initialize` 必须返回能力矩阵，GUI 不能猜后端能力 |
| failure is explicit | degraded、failed、version_mismatch、policy_unavailable 等状态必须结构化 |

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
| `--self-check` | initialize + lifecycle + health + capabilities + schema + in-memory session smoke report | local smoke probe before wiring Electron sidecar supervision |

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
| `protocol` | implemented | `initialize`、`protocol/schema` | none |
| `lifecycle` | implemented | `lifecycle/status`、`shutdown` | `lifecycle/changed` |
| `health` | implemented | `health/check`、`capabilities/list` | `health/changed`、`capabilities/changed` |
| `logs` | declared | none | `log/entry` |
| `session` | declared | `thread/create`、`thread/list`、`thread/read`、`turn/start`、`turn/cancel`、`turn/list`、`turn/read` | `thread/created`、`turn/started`、`turn/cancelled` |
| `approval` | declared | none | none |
| `dlpPolicy` | declared | none | none |
| `modelProvider` | declared | none | none |
| `tools` | declared | none | none |
| `jobs` | declared | none | none |
| `skills` | declared | none | none |
| `mcp` | declared | none | none |
| `sandbox` | declared | none | none |

Rule: `implemented` means every advertised method has a handler. A future capability may be `declared` to reserve protocol shape, but the GUI must not call missing methods.

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
  unavailableRequestedCapabilities?: string[];
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
| session | `degraded`; in-memory thread/turn host is available, runtime turn execution is not wired |
| logs | `ready` |
| runtime | `unavailable` or `disabled` until runtime bridge is wired |
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

### 5.7 `thread/create` skeleton

```ts
type ThreadCreateParams = {
  title?: string;
  workspaceRoot?: string;
};

type ThreadCreateResponse = {
  threadId: string;
  lifecycle: LifecycleSnapshot;
};
```

Phase 1.5 behavior: after `initialize`, app-server creates an in-memory thread id and emits `thread/created`. Before `initialize`, it returns `NOT_INITIALIZED` with `retryable=true`. The session capability remains `declared` because turns are not backed by the runtime Agent bridge yet.

### 5.8 `thread/list` skeleton

```ts
type ThreadSummary = {
  threadId: string;
  title?: string;
  workspaceRoot?: string;
};

type ThreadListResponse = {
  threads: ThreadSummary[];
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
  prompt: string;
};

type TurnStartResponse = {
  turnId: string;
  status: "pending" | "cancelled";
  lifecycle: LifecycleSnapshot;
};
```

Phase 1.5 behavior: app-server first enforces `initialize` and validates that `threadId` exists. If both pass, it creates an in-memory pending turn and emits `turn/started`. It does not call the runtime Agent bridge or model provider yet.

### 5.11 `turn/cancel` skeleton

```ts
type TurnCancelParams = {
  threadId: string;
  turnId: string;
};

type TurnCancelResponse = {
  accepted: boolean;
  status: "pending" | "cancelled";
  lifecycle: LifecycleSnapshot;
};
```

Phase 1.5 behavior: app-server first enforces `initialize` and validates that `threadId` exists. Known pending turns are marked `cancelled` and emit `turn/cancelled`; unknown turns return `INVALID_PARAMS`. No runtime cancellation primitive is wired yet.

### 5.12 `turn/list` skeleton

```ts
type TurnSummary = {
  threadId: string;
  turnId: string;
  status: "pending" | "cancelled";
};

type TurnListParams = {
  threadId: string;
};

type TurnListResponse = {
  turns: TurnSummary[];
};
```

Phase 1.5 behavior: after `initialize`, app-server validates `threadId` and returns in-memory turn summaries for that thread. Unknown threads return `INVALID_PARAMS`. Prompt/message history is not exposed.

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

Phase 1.5 behavior: after `initialize`, app-server validates `threadId` and returns one in-memory turn summary. Unknown turns return `INVALID_PARAMS`. Runtime execution output is not implemented.

## 6. Events v0

### 6.1 `lifecycle/changed`

```ts
type LifecycleChangedEvent = {
  lifecycle: LifecycleSnapshot;
  previousState?: LifecycleState;
};
```

### 6.2 `health/changed`

```ts
type HealthChangedEvent = {
  ok: boolean;
  services: ServiceHealth[];
};
```

### 6.3 `capabilities/changed`

```ts
type CapabilitiesChangedEvent = {
  capabilities: CapabilityMatrix;
  reason: "initialize" | "config_changed" | "service_degraded" | "service_recovered";
};
```

### 6.4 `log/entry`

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

### 6.5 Session skeleton events

```ts
type ThreadCreatedEvent = {
  threadId: string;
};

type TurnStartedEvent = {
  threadId: string;
  turnId: string;
  status: "pending" | "cancelled";
};

type TurnCancelledEvent = {
  threadId: string;
  turnId: string;
  status: "pending" | "cancelled";
};
```

Phase 1.5 behavior: app-server emits `thread/created` for in-memory thread creation, `turn/started` for in-memory pending turns, and `turn/cancelled` for pending-turn cancellation. These events do not imply runtime Agent execution.

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
| session/thread | `thread/create`、`thread/send`、`thread/cancel`、`thread/event` |
| approval | `approval/respond`、`approval/requested`、`approval/resolved` |
| chat stream | `message/delta`、`reasoning/delta`、`turn/completed` |
| tools | `tool/started`、`tool/output_delta`、`tool/completed` |
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
