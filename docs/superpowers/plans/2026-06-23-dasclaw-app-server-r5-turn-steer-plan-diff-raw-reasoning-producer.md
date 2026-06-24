# Dasclaw App Server R5 Turn Steer Plan Diff Raw Reasoning Producer Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 完成 `docs/plans/dasclaw-app-server-codex-protocol-gap-matrix.md` 的 R5：`turn/steer`、`turn/plan/updated`、`turn/diff/updated`、`item/plan/delta`、`rawResponseItem/completed`、`item/reasoning/summaryPartAdded` producer、`item/reasoning/textDelta` producer。完成标准是 app-server 有真实 route / producer / capability / profile / tests；客户端 tracker 不依赖测试 fake；未被默认 runtime 支持的能力必须 fail-safe 或不广告。

**Architecture:** R5 分三层落地。协议层在 `dasclaw_app_server_protocol` 补 Codex v2 wire shape 和 schema fixture；runtime/provider 层把 provider stream 与 `AgentEvent` 扩展成可携带 summary part、raw reasoning、raw response item、plan delta、turn plan snapshot、turn diff snapshot 的事件；app-server 层把 `RuntimeTurnUpdateSink` 事件转换成 `ServerNotification`，并新增 `turn/steer` route，通过 `RuntimeBridgeFeatures` 只在运行时真支持时广告。compatibility profile 与 gap matrix 只在 route 和 producer 受测通过后移除对应 opt-out / 删除线。

**Tech Stack:** Rust workspace；`dasclaw_app_server_protocol` JSON-RPC protocol DTO；`dasclaw_app_server` route / notification bus / runtime bridge；`dasclaw_runtime` `AgentEvent`；`dasclaw_llm_provider` `LlmStreamEvent`；`cargo nextest`、`cargo check`、`cargo fmt`、`check_no_panics.py`。

---

## 0. 启动核验与证据

仓库 4 问结果：

| 问题 | 结论 | R5 处理 |
|---|---|---|
| 是否新增模块 / crate / 文件？ | 是，新增本执行计划；实现可能新增 DTO / enum / 测试 | 已先做语义搜索、LSP、`rg`，确认不是重复计划 |
| 是否包含否定结论？ | 是，会判断当前 app-server 缺 producer / route | 用 Level 1 + Level 2 + Level 3 证据限定结论 |
| 是否跨项目对账？ | 是，对照 `codex-cli-main` 与 `crates/dasclaw_*` | 本计划列出 Codex 参考落点 |
| 是否写架构对账文档？ | 是，R5 是协议差距计划 | 只写实证状态，不把仅 schema 当完成 |

三层核验证据：

| 层级 | 证据 | 结论 |
|---|---|---|
| Level 1 语义层 | `semantic_search_nodes_tool` 查询 “turn steer / plan delta / raw response item / reasoning text delta producer” 与 “runtime event producer maps reasoning raw content delta plan update turn diff notification” | `codex-cli-main` 命中 `turn_steer`、`handle_turn_plan_update`、`handle_turn_diff`、`maybe_emit_raw_response_item_completed`；当前 `crates` 命中 provider/runtime 基础流，但未命中 Dasclaw app-server R5 owner |
| Level 2 符号层 | `lsp-mcp execute_lsp document_symbols` 查询 Codex `codex_message_processor.rs`、`bespoke_event_handling.rs`，以及 Dasclaw `dasclaw_app_server/src/lib.rs`、`dasclaw_app_server_protocol/src/lib.rs` | Codex 有 `turn_steer`、`handle_turn_plan_update`、`handle_turn_diff`、`maybe_emit_raw_response_item_completed`；Dasclaw protocol 仅已有 `ReasoningSummaryPartAddedEvent` / `ReasoningTextDeltaEvent`，app-server 无 `turn_steer` 和 plan/diff/raw producer |
| Level 3 字面层 | `rg` 精确查 `turn/steer`、`TURN_STEER`、`turn/plan/updated`、`turn/diff/updated`、`rawResponseItem/completed`、`item/plan/delta`、`reasoning/textDelta` | `crates/dasclaw_protocol` 已有 `PlanDeltaEvent`、`ReasoningRawContentDeltaEvent`、`RawResponseItemEvent`、`TurnDiffEvent`；`crates/dasclaw_app_server_protocol` 已有 reasoning summary part/text DTO；`crates/dasclaw_app_server` 当前只稳定产出 `summaryTextDelta`，未产出 R5 新事件 |

Codex 参考落点：

| 能力 | Codex 参考 |
|---|---|
| `turn/steer` | `codex-cli-main/codex-rs/app-server/src/codex_message_processor.rs:7346`；协议 DTO 在 `codex-rs/app-server-protocol/src/protocol/v2.rs:5306` |
| turn plan snapshot | `codex-cli-main/codex-rs/app-server/src/bespoke_event_handling.rs:2021`；协议 DTO 在 `v2.rs:6444` |
| turn diff snapshot | `codex-cli-main/codex-rs/app-server/src/bespoke_event_handling.rs:2002`；字段名是 `diff` |
| raw response item | `codex-cli-main/codex-rs/app-server/src/bespoke_event_handling.rs:2196`；协议 DTO 在 `v2.rs:6561` |
| item plan delta | `codex-cli-main/codex-rs/app-server/src/bespoke_event_handling.rs:1387`；协议 DTO 在 `v2.rs:6583` |
| reasoning summary part / raw text delta | `codex-cli-main/codex-rs/core/src/session/turn.rs` 将 `ReasoningSummaryPartAdded`、`ReasoningContentDelta` 转成 v2 notification |

过程透明记录：已检查 R5 turn steer / plan / diff / raw reasoning producer 是否已有，结论：Codex app-server 有完整参考；Dasclaw 已有部分协议词汇和 reasoning DTO，但 app-server 当前没有 `turn/steer`、plan/diff/raw producer owner，summary part/text delta 也尚未由 runtime bridge 真实产出。

## 0.1 2026-06-24 复审校准：R5 尚未整体完成

2026-06-24 子代理复审结论：

| Review lane | 结论 | 对 R5 gap matrix 的影响 |
|---|---|---|
| `code-reviewer` | `REQUEST CHANGES`，3 个 HIGH | 不可以把 R5 整体划掉 |
| `architect` | `BLOCK` | 不可以把 R5 整体划掉 |

复审后的真实状态：

| 范围 | 状态 | 证据 / 边界 |
|---|---|---|
| 协议 DTO / schema / notification constructors | 已完成 | `TurnSteerParams`、`TurnPlanUpdatedEvent`、`TurnDiffUpdatedEvent`、`PlanDeltaEvent`、`RawResponseItemCompletedEvent` 已接入 protocol tests |
| app-server route / fail-safe | 部分完成 | `turn/steer` route、active turn precondition、feature-disabled `CAPABILITY_UNAVAILABLE` 已受测；但真实 `DasclawAgentRuntimeBridge` 仍不声明 `turn_steer: true` |
| runtime adapter 接收 R5 event 后转发 | 部分完成 | `LlmStreamEvent` -> `AgentEvent` 与 `AgentEvent` -> `ServerNotification` 受测；但测试主要使用 scripted provider / synthetic bridge |
| 真实 provider producer | 未完成 | `claw_code_provider` / `codex_chatgpt` 真实解析路径仍只稳定产出 text / reasoning summary / tool call / completed 等旧事件，未稳定产出 R5 plan/diff/raw item/raw reasoning events |
| 真实 turn steer 注入运行中 turn | 未完成 | trait 默认 `steer_turn` 仍 unsupported；真实 bridge 没有把 steer input 注入 running turn 的 API / test |
| gap matrix R5 删除线 | 不允许整体划掉 | 只能标注 route/schema/adapter 接收路径已完成；真实 provider/runtime producer 和真实 steer 仍是剩余项 |

阻塞项：

1. **真实 provider 侧没有产出 R5 事件。** 当前新增事件主要出现在 enum、adapter 映射和测试中；不能把 scripted provider 通过写成真实 provider producer 完成。
2. **`turn/steer` 只有 fail-safe route 和 fake bridge success path。** 这条边界是诚实的，但不等于真实 runtime steer 能力完成。
3. **测试覆盖证明接收链路，不证明真实生产链路。** 后续必须补 provider fixture/parser 测试或真实 runtime fixture，避免测试侧适配掩盖客户端/协议缺口。
4. **`provider/reasoning.rs` 对新增 R5 events 的处理需要显式化。** 如果 wrapper 无法处理这些事件，必须写明 fail-safe/ignored rationale，不能让新增事件被无声吞掉后还声称 producer 完成。

本计划后续执行原则：

- 不得把 `docs/plans/dasclaw-app-server-codex-protocol-gap-matrix.md` 的 R5 整体划掉，直到 Task 8-10 的真实 producer / steer 工作完成并通过验证。
- 可以在 gap matrix 中拆分记录：`R5a protocol/router/adapter receive path` 已完成；`R5b real provider producer` 与 `R5c real turn steer injection` 未完成。
- 所有新增测试必须至少包含一个非 scripted-provider 的 fixture/parser 级用例；只用 `ScriptedEventsProvider`、`ScriptedRuntimeResponder`、`SequencedRuntimeBridge` 的测试不能作为 R5 完成证据。

## 1. 范围

In scope：

- `turn/steer` JSON-RPC method：参数、route、active turn precondition、expected turn id mismatch、runtime unsupported fail-safe、runtime-supported success path。
- `turn/plan/updated` 与 `turn/diff/updated`：turn-level snapshot notification。
- `item/plan/delta`：item-level proposed plan delta notification。
- `rawResponseItem/completed`：raw provider/Responses item completed notification，payload 保持 raw `serde_json::Value`，不伪装成 `CodexThreadItem`。
- `item/reasoning/summaryPartAdded` 与 `item/reasoning/textDelta`：从 runtime/provider 真实事件进入 app-server producer。
- Capability matrix、schema、supported methods、compatibility profile、gap matrix 更新。

Out of scope：

- Codex Realtime、account、plugin marketplace、guardian denied action、review/git/fuzzy search。
- 把 raw chain-of-thought 展示策略改成默认开启。R5 只补 raw reasoning transport；是否暴露由 provider/runtime policy 与 client 决定。
- 完整 plan-mode 产品 UI。R5 只负责 app-server protocol producer 和 route owner。

## 2. File Structure

需要修改的文件：

- `crates/dasclaw_app_server_protocol/src/lib.rs` - R5 method/event constants、DTO、notification constructors、schema/profile fixtures。
- `crates/dasclaw_app_server/src/lib.rs` - route、supported methods、runtime bridge traits/features、notification bus、drain producer、tests。
- `crates/dasclaw_core/src/agentic_loop.rs` - `AgentEvent` 增加 R5 streaming event vocabulary。
- `crates/dasclaw_runtime/src/llm_adapter.rs` - provider stream 到 `AgentEvent` 的映射。
- `crates/dasclaw_llm_provider/src/provider/provider.rs` - `LlmStreamEvent` 增加 provider-level R5 event。
- provider 实现文件（至少 `crates/dasclaw_llm_provider/src/provider/openai_codex_provider.rs`、`crates/dasclaw_llm_provider/src/providers/openai_compat.rs`，按实际调用链确认）- 转发真实 SSE 中能拿到的 raw item / reasoning / plan / diff 事件；不能拿到时保持不产出，不做 fake。
- `docs/plans/dasclaw-app-server-codex-protocol-gap-matrix.md` - R5 完成态与剩余风险记录。

## 3. Task 1 - 锁定 R5 协议合约

- [x] Red：在 `crates/dasclaw_app_server_protocol/src/lib.rs` 增加协议测试，先断言缺失的 method/event/DTO。

测试名建议：

```rust
#[test]
fn r5_turn_steer_and_streaming_events_use_codex_v2_wire_shapes() {
    assert_eq!(method::TURN_STEER, "turn/steer");
    assert_eq!(event::TURN_PLAN_UPDATED, "turn/plan/updated");
    assert_eq!(event::TURN_DIFF_UPDATED, "turn/diff/updated");
    assert_eq!(event::ITEM_PLAN_DELTA, "item/plan/delta");
    assert_eq!(event::RAW_RESPONSE_ITEM_COMPLETED, "rawResponseItem/completed");
}
```

补 payload fixture，字段名必须和 Codex v2 对齐：

```rust
let steer: TurnSteerParams = serde_json::from_value(serde_json::json!({
    "threadId": "thread_1",
    "expectedTurnId": "turn_1",
    "input": [{ "type": "text", "text": "continue with the safer option" }]
}))?;

let plan = serde_json::to_value(TurnPlanUpdatedEvent {
    thread_id: "thread_1".into(),
    turn_id: "turn_1".into(),
    explanation: Some("Adjusting plan".into()),
    plan: vec![TurnPlanStep {
        step: "inspect current producer".into(),
        status: TurnPlanStepStatus::InProgress,
    }],
})?;
assert_eq!(plan["plan"][0]["status"], "inProgress");

let diff = serde_json::to_value(TurnDiffUpdatedEvent {
    thread_id: "thread_1".into(),
    turn_id: "turn_1".into(),
    diff: "--- a/file\n+++ b/file\n".into(),
})?;
assert!(diff.get("unifiedDiff").is_none());
assert_eq!(diff["diff"], "--- a/file\n+++ b/file\n");
```

- [x] Green：添加协议常量和 DTO。

目标 shape：

```rust
pub mod method {
    pub const TURN_STEER: &str = "turn/steer";
}

pub mod event {
    pub const TURN_PLAN_UPDATED: &str = "turn/plan/updated";
    pub const TURN_DIFF_UPDATED: &str = "turn/diff/updated";
    pub const RAW_RESPONSE_ITEM_COMPLETED: &str = "rawResponseItem/completed";
    pub const ITEM_PLAN_DELTA: &str = "item/plan/delta";
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnSteerParams {
    pub thread_id: String,
    pub input: Vec<UserInput>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub responsesapi_client_metadata: Option<HashMap<String, String>>,
    pub expected_turn_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnSteerResponse {
    pub turn_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnPlanUpdatedEvent {
    pub thread_id: String,
    pub turn_id: String,
    pub explanation: Option<String>,
    pub plan: Vec<TurnPlanStep>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnPlanStep {
    pub step: String,
    pub status: TurnPlanStepStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TurnPlanStepStatus {
    Pending,
    InProgress,
    Completed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnDiffUpdatedEvent {
    pub thread_id: String,
    pub turn_id: String,
    pub diff: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawResponseItemCompletedEvent {
    pub thread_id: String,
    pub turn_id: String,
    pub item: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanDeltaEvent {
    pub thread_id: String,
    pub turn_id: String,
    pub item_id: String,
    pub delta: String,
}
```

- [x] Green：给 `ServerNotification` 添加 constructors：

```rust
pub fn turn_plan_updated(event: TurnPlanUpdatedEvent) -> Result<Self, serde_json::Error> {
    Self::new(event::TURN_PLAN_UPDATED, event)
}

pub fn turn_diff_updated(event: TurnDiffUpdatedEvent) -> Result<Self, serde_json::Error> {
    Self::new(event::TURN_DIFF_UPDATED, event)
}

pub fn raw_response_item_completed(
    event: RawResponseItemCompletedEvent,
) -> Result<Self, serde_json::Error> {
    Self::new(event::RAW_RESPONSE_ITEM_COMPLETED, event)
}

pub fn plan_delta(event: PlanDeltaEvent) -> Result<Self, serde_json::Error> {
    Self::new(event::ITEM_PLAN_DELTA, event)
}
```

- [x] Refactor：保持 `UserInput` 复用现有 `turn/start` 输入解析，不新增并行 input type。

## 4. Task 2 - 扩展 runtime/provider 事件词汇

- [x] Red：在 `crates/dasclaw_runtime/src/llm_adapter.rs` 增加 adapter tests，证明 provider R5 stream event 会变成 agent R5 event；先让测试失败。

测试名建议：

- `provider_plan_delta_streams_to_agent_event`
- `provider_reasoning_raw_delta_streams_to_agent_event`
- `provider_raw_response_item_completed_streams_to_agent_event`
- `legacy_reasoning_tag_still_emits_summary_chunk_only`

- [x] Green：在 `crates/dasclaw_llm_provider/src/provider/provider.rs` 增加 provider event enum。不要在 provider 层制造 `thread_id` / `turn_id`，这些由 app-server runtime bridge 注入。

```rust
pub enum LlmStreamEvent {
    TextDelta(String),
    ReasoningSummaryDelta(String),
    ReasoningSummaryPartAdded {
        item_id: Option<String>,
        summary_index: i64,
    },
    ReasoningRawTextDelta {
        item_id: Option<String>,
        content_index: i64,
        delta: String,
    },
    PlanDelta {
        item_id: Option<String>,
        delta: String,
    },
    TurnPlanUpdated {
        explanation: Option<String>,
        plan: Vec<LlmPlanStep>,
    },
    TurnDiffUpdated {
        diff: String,
    },
    RawResponseItemCompleted {
        item: serde_json::Value,
    },
    ToolCallInputDelta { id: String, name: Option<String>, delta: String },
    Completed(ToolCompletionResponse),
}
```

新增 provider plan step type：

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LlmPlanStep {
    pub step: String,
    pub status: LlmPlanStepStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LlmPlanStepStatus {
    Pending,
    InProgress,
    Completed,
}
```

- [x] Green：在 `crates/dasclaw_core/src/agentic_loop.rs` 增加 app-runtime 可消费的 `AgentEvent`。

```rust
pub enum AgentEvent {
    TextChunk(String),
    ReasoningSummaryChunk(String),
    ReasoningSummaryPartAdded {
        item_id: Option<String>,
        summary_index: i64,
    },
    ReasoningRawTextChunk {
        item_id: Option<String>,
        content_index: i64,
        delta: String,
    },
    PlanDelta {
        item_id: Option<String>,
        delta: String,
    },
    TurnPlanUpdated {
        explanation: Option<String>,
        plan: Vec<AgentPlanStep>,
    },
    TurnDiffUpdated {
        diff: String,
    },
    RawResponseItemCompleted {
        item: serde_json::Value,
    },
    // existing variants...
}
```

`AgentPlanStep` / `AgentPlanStepStatus` 可以放在 `agentic_loop.rs`，避免 `dasclaw_core` 依赖 app-server protocol。

- [x] Green：在 `LlmProviderResponder::stream_provider_response` 中逐项映射 provider event 到 agent event。

```rust
match event? {
    LlmStreamEvent::ReasoningSummaryPartAdded { item_id, summary_index } => {
        if emit_reasoning && let Some(event_tx) = event_tx.as_ref() {
            let _ = event_tx
                .send(AgentEvent::ReasoningSummaryPartAdded { item_id, summary_index })
                .await;
        }
    }
    LlmStreamEvent::ReasoningRawTextDelta { item_id, content_index, delta } => {
        if emit_reasoning && let Some(event_tx) = event_tx.as_ref() {
            let _ = event_tx
                .send(AgentEvent::ReasoningRawTextChunk { item_id, content_index, delta })
                .await;
        }
    }
    LlmStreamEvent::PlanDelta { item_id, delta } => {
        if let Some(event_tx) = event_tx.as_ref() {
            let _ = event_tx.send(AgentEvent::PlanDelta { item_id, delta }).await;
        }
    }
    // map turn plan / diff / raw item similarly
    existing => { /* keep current behavior */ }
}
```

- [x] Refactor：不要让 `LegacyReasoningStream` 产生 raw reasoning text delta。它从 text tags 推断 summary，只能继续产出 `ReasoningSummaryChunk`，否则会把模型普通文本误标成 raw reasoning。

## 5. Task 3 - 实现 `turn/steer` app-server owner 与 runtime contract

- [x] Red：在 `crates/dasclaw_app_server/src/lib.rs` 增加 route tests。

覆盖：

- `turn_steer_rejects_missing_active_turn`
- `turn_steer_rejects_expected_turn_mismatch`
- `turn_steer_rejects_empty_text_input`
- `turn_steer_rejects_when_runtime_feature_disabled`
- `turn_steer_forwards_to_runtime_bridge_and_returns_turn_id`
- `supported_methods_only_advertises_turn_steer_when_runtime_supports_it`

- [x] Green：扩展 runtime bridge feature 和 request。

```rust
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RuntimeBridgeFeatures {
    pub turn_steer: bool,
    // existing flags...
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeTurnSteerRequest {
    pub thread_id: String,
    pub turn_id: String,
    pub input: Vec<UserInput>,
    pub prompt: String,
    pub responsesapi_client_metadata: Option<HashMap<String, String>>,
}

pub trait RuntimeBridge: std::fmt::Debug + Send + Sync {
    fn steer_turn(&self, _request: RuntimeTurnSteerRequest) -> Result<String, RuntimeBridgeError> {
        Err(RuntimeBridgeError::fatal("runtime bridge does not support turn steer"))
    }
}
```

- [x] Green：在 `AppServer::turn_steer` 中实现 fail-safe precondition。

```rust
pub fn turn_steer(
    &mut self,
    params: TurnSteerParams,
) -> Result<TurnSteerResponse, AppServerError> {
    self.require_initialized("session")?;
    let prompt = text_prompt_from_user_input(&params.input, "turn/steer")?;
    if prompt.trim().is_empty() {
        return Err(AppServerError::invalid_request(
            "turn/steer",
            "turn/steer input must include at least one text item",
        ));
    }
    if !self.threads.turn_is_pending(&params.thread_id, &params.expected_turn_id) {
        return Err(AppServerError::invalid_request(
            "turn/steer",
            "expectedTurnId does not match the active turn",
        ));
    }
    if !self.runtime_features.turn_steer {
        return Err(AppServerError::capability_unavailable(
            "turn_steer",
            "runtime bridge does not support turn steer",
        ));
    }
    let turn_id = self.runtime_bridge
        .steer_turn(RuntimeTurnSteerRequest {
            thread_id: params.thread_id,
            turn_id: params.expected_turn_id,
            input: params.input,
            prompt,
            responsesapi_client_metadata: params.responsesapi_client_metadata,
        })
        .map_err(AppServerError::runtime_bridge)?;
    Ok(TurnSteerResponse { turn_id })
}
```

- [x] Green：route JSON-RPC method `method::TURN_STEER` 到 `turn_steer`；`supported_methods()` 与 schema/capability 只在 runtime feature 支持时广告。

- [x] Green：为真实 `DasclawAgentRuntimeBridge` 做明确选择：
  - 若 `dasclaw_runtime::Agent` 已有运行中追加用户输入 API，则 `steer_turn` 调用该 API。
  - 若没有，默认 `DasclawAgentRuntimeBridge` 不声明 `turn_steer`，route 保持 fail-safe；R5 不能把 `turn/steer` 从 gap matrix 删除线标完成，直到真实 bridge API 存在并通过测试。

- [x] Refactor：错误语义优先映射到现有 `AppServerError::invalid_request` / `capability_unavailable`，不要新增只用于测试的错误码。

## 6. Task 4 - 接入 R5 notification producer

- [x] Red：在 `crates/dasclaw_app_server/src/lib.rs` 增加 producer tests。使用 test runtime bridge 往 `RuntimeTurnUpdateSink` 推事件，再通过 `drain_json_rpc_notifications_with_policy()` 验证 JSON-RPC notification。

测试名建议：

- `runtime_turn_update_emits_reasoning_summary_part_added`
- `runtime_turn_update_emits_reasoning_text_delta`
- `runtime_turn_update_emits_plan_delta`
- `runtime_turn_update_emits_turn_plan_updated`
- `runtime_turn_update_emits_turn_diff_updated`
- `runtime_turn_update_emits_raw_response_item_completed`
- `r5_updates_are_ignored_after_turn_is_no_longer_pending`

- [x] Green：新增 runtime update payloads。

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeReasoningSummaryPartAddedUpdate {
    pub item_id: Option<String>,
    pub summary_index: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeReasoningTextDeltaUpdate {
    pub item_id: Option<String>,
    pub content_index: i64,
    pub delta: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimePlanDeltaUpdate {
    pub item_id: Option<String>,
    pub delta: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeTurnPlanUpdatedUpdate {
    pub explanation: Option<String>,
    pub plan: Vec<TurnPlanStep>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeTurnDiffUpdatedUpdate {
    pub diff: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeRawResponseItemCompletedUpdate {
    pub item: serde_json::Value,
}
```

Add enum variants:

```rust
pub enum RuntimeTurnOutcome {
    ReasoningSummaryPartAdded { update: RuntimeReasoningSummaryPartAddedUpdate },
    ReasoningTextDelta { update: RuntimeReasoningTextDeltaUpdate },
    PlanDelta { update: RuntimePlanDeltaUpdate },
    TurnPlanUpdated { update: RuntimeTurnPlanUpdatedUpdate },
    TurnDiffUpdated { update: RuntimeTurnDiffUpdatedUpdate },
    RawResponseItemCompleted { update: RuntimeRawResponseItemCompletedUpdate },
    // existing variants...
}
```

- [x] Green：在 `RuntimeTurnUpdateSink` 增加 public methods。

```rust
pub fn reasoning_summary_part_added(
    &self,
    thread_id: String,
    turn_id: String,
    update: RuntimeReasoningSummaryPartAddedUpdate,
) {
    self.push(RuntimeTurnUpdate {
        thread_id,
        turn_id,
        outcome: RuntimeTurnOutcome::ReasoningSummaryPartAdded { update },
    });
}

pub fn reasoning_text_delta(
    &self,
    thread_id: String,
    turn_id: String,
    update: RuntimeReasoningTextDeltaUpdate,
) { /* same pattern */ }
```

- [x] Green：在 `NotificationBus` 增加 emit wrappers。

```rust
pub fn emit_reasoning_summary_part_added(&mut self, event: ReasoningSummaryPartAddedEvent) {
    self.push(ServerNotification::reasoning_summary_part_added(event));
}

pub fn emit_reasoning_text_delta(&mut self, event: ReasoningTextDeltaEvent) {
    self.push(ServerNotification::reasoning_text_delta(event));
}

pub fn emit_turn_plan_updated(&mut self, event: TurnPlanUpdatedEvent) {
    self.push(ServerNotification::turn_plan_updated(event));
}
```

- [x] Green：在 `drain_runtime_turn_updates` 中只对 pending turn 发流式事件。

```rust
RuntimeTurnOutcome::ReasoningTextDelta { update: reasoning } => {
    if self.threads.turn_is_pending(&update.thread_id, &update.turn_id) {
        let item_id = reasoning
            .item_id
            .unwrap_or_else(|| runtime_reasoning_item_id(&update.turn_id));
        self.notifications.emit_reasoning_text_delta(ReasoningTextDeltaEvent {
            thread_id: update.thread_id,
            turn_id: update.turn_id,
            item_id,
            content_index: reasoning.content_index,
            delta: reasoning.delta,
        });
    }
}
```

同样接入 `ReasoningSummaryPartAdded`、`PlanDelta`、`TurnPlanUpdated`、`TurnDiffUpdated`、`RawResponseItemCompleted`。其中 `RawResponseItemCompleted` 不写入 `ThreadLifecycleHost.items`，除非 payload 能安全转换成 `CodexThreadItem`；默认只发通知，避免历史视图混入 raw provider 结构。

- [x] Refactor：复用已有 `runtime_agent_item_id` / `runtime_tool_item_id` 风格 helper，新增 `runtime_reasoning_item_id(turn_id)` 与 `runtime_plan_item_id(turn_id)`，保持 item id 稳定。

## 7. Task 5 - 将真实 bridge 事件转成 R5 runtime updates

- [x] Red：为 `DasclawAgentRuntimeBridge::start_turn` 增加 tests，构造 scripted agent stream，验证每类 `AgentEvent` 都进入 `RuntimeTurnUpdateSink`。

测试名建议：

- `dasclaw_runtime_bridge_forwards_reasoning_summary_part_added`
- `dasclaw_runtime_bridge_forwards_reasoning_text_delta`
- `dasclaw_runtime_bridge_forwards_plan_delta`
- `dasclaw_runtime_bridge_forwards_turn_plan_and_diff`
- `dasclaw_runtime_bridge_forwards_raw_response_item_completed`

- [x] Green：更新 `start_turn` event loop。

```rust
Ok(dasclaw_runtime::AgentEvent::ReasoningSummaryPartAdded {
    item_id,
    summary_index,
}) => {
    event_updates.reasoning_summary_part_added(
        event_thread_id.clone(),
        event_turn_id.clone(),
        RuntimeReasoningSummaryPartAddedUpdate { item_id, summary_index },
    );
}
Ok(dasclaw_runtime::AgentEvent::ReasoningRawTextChunk {
    item_id,
    content_index,
    delta,
}) => {
    if !delta.is_empty() {
        event_updates.reasoning_text_delta(
            event_thread_id.clone(),
            event_turn_id.clone(),
            RuntimeReasoningTextDeltaUpdate { item_id, content_index, delta },
        );
    }
}
Ok(dasclaw_runtime::AgentEvent::PlanDelta { item_id, delta }) => {
    if !delta.is_empty() {
        event_updates.plan_delta(
            event_thread_id.clone(),
            event_turn_id.clone(),
            RuntimePlanDeltaUpdate { item_id, delta },
        );
    }
}
```

- [x] Green：对 provider 实现逐个接线。
  - OpenAI Responses / Codex provider 能拿到 raw response item completed 时，直接产出 `LlmStreamEvent::RawResponseItemCompleted { item }`。
  - 能拿到 reasoning raw text delta 时，产出 `ReasoningRawTextDelta`；summary-only provider 继续只产出 `ReasoningSummaryDelta`。
  - plan/diff 如果来自 tool 或 session event，则从对应 owner 产出 `TurnPlanUpdated` / `TurnDiffUpdated`；provider 没有这类事件时不伪造。

- [x] Refactor：删除或收紧 `Ok(_) => {}` 的无声吞吐。保留 wildcard 时必须有注释说明哪些事件由上层故意忽略，避免以后新增事件被静默丢弃。

## 8. Task 6 - Capability、schema、profile 与 gap matrix

- [x] Red：补 capability/profile tests。

测试名建议：

- `r5_supported_methods_and_schema_include_turn_steer_only_when_runtime_supports_it`
- `r5_profile_includes_only_real_producers`
- `compatibility_profile_does_not_opt_out_completed_r5_plan_and_diff`

- [x] Green：更新 `supported_methods()` 和 router。

如果 `supported_methods()` 是静态列表，则 `turn/steer` 可以出现在 schema 的 declared method 中，但 `CapabilityMatrix::phase_one()` / initialize response 里的 implemented capability 必须受 `runtime_features.turn_steer` gating。若当前测试要求 supported methods 等于 routable methods，则把 `turn/steer` route 做成 fail-safe `CAPABILITY_UNAVAILABLE`，并在 capability 中标为 unavailable，不标 implemented。

- [x] Green：更新 capability matrix。

完成后 session capability 至少包含：

```rust
methods: [
    method::TURN_START,
    method::TURN_INTERRUPT,
    method::TURN_READ,
    // method::TURN_STEER only when runtime_features.turn_steer
],
events: [
    event::TURN_PLAN_UPDATED,
    event::TURN_DIFF_UPDATED,
    event::ITEM_PLAN_DELTA,
    event::RAW_RESPONSE_ITEM_COMPLETED,
    event::ITEM_REASONING_SUMMARY_PART_ADDED,
    event::ITEM_REASONING_TEXT_DELTA,
]
```

- [x] Green：更新 `CompatibilityProfile::codex_app_server_v2_chat_session_subset()`：
  - 已完成 producer 的 `codex.plan` / `codex.diff` opt-out 移除。
  - 若 `turn/steer` 默认 runtime 仍不支持，保留 steer opt-out 或 feature-gated 描述，不写成默认完成。
  - 保留 `rawResponseItem/completed` 的 internal-only / unstable 说明。

- [x] Green：更新 `docs/plans/dasclaw-app-server-codex-protocol-gap-matrix.md` R5 行和 §6 相关表格：
  - 只有 route / producer / tests / profile 都完成的项加删除线。
  - 明确 `turn/plan/updated` 是 `update_plan` checklist snapshot，不等同 `item/plan/delta` 的 proposed-plan streaming。
  - 明确 raw response item 默认只 notification，不写 thread history。

## 9. Task 7 - Verification 与自审

- [x] 运行协议 crate check：

```bash
CARGO_TARGET_DIR=target/codex-r5-verify RUSTC_WRAPPER= \
  cargo check -p dasclaw_app_server_protocol --tests
```

- [x] 运行 app-server check：

```bash
CARGO_TARGET_DIR=target/codex-r5-verify RUSTC_WRAPPER= \
  cargo check -p dasclaw_app_server --tests
```

- [x] 运行 runtime / provider check（如果改了这些 crate）：

```bash
CARGO_TARGET_DIR=target/codex-r5-verify RUSTC_WRAPPER= \
  cargo check -p dasclaw_runtime -p dasclaw_llm_provider --tests
```

- [x] 运行 targeted nextest：

```bash
CARGO_TARGET_DIR=target/codex-r5-verify RUSTC_WRAPPER= \
  cargo nextest run \
    -p dasclaw_app_server_protocol \
    -p dasclaw_app_server \
    -p dasclaw_runtime \
    -p dasclaw_llm_provider \
    -E 'test(r5_) | test(turn_steer_) | test(plan_delta_) | test(turn_plan_) | test(turn_diff_) | test(raw_response_) | test(reasoning_text_delta_) | test(reasoning_summary_part_)'
```

- [x] 运行格式和 panic guard：

```bash
cargo fmt --all && python3.12 scripts/check_no_panics.py --base origin/xClaw
```

- [x] 运行 touched crate clippy：

```bash
CARGO_TARGET_DIR=target/codex-r5-verify RUSTC_WRAPPER= \
  cargo clippy --no-deps \
    -p dasclaw_app_server_protocol \
    -p dasclaw_app_server \
    -p dasclaw_runtime \
    -p dasclaw_llm_provider \
    --all-targets -- -D warnings
```

- [x] 执行仓库要求的自查顺序：
  - `code-quality-audit`
  - `code-simplifier`（只对本轮新代码做收敛；不要改动 verbatim port 段）
  - 若触及 `crates/dasclaw_*` 与 compatibility profile，PR push 前执行 `adr-compliance-check`
  - PR 自审前执行 `code-review-expert`

## 10. Acceptance Criteria

- [x] `turn/steer` 有 JSON-RPC route，支持 active turn / expected turn id / empty input / runtime unsupported / runtime success 测试。
- [x] `item/reasoning/summaryPartAdded` 与 `item/reasoning/textDelta` 由真实 runtime bridge event 产出，不只停留在 protocol constructor。
- [x] `turn/plan/updated`、`turn/diff/updated`、`item/plan/delta`、`rawResponseItem/completed` 均有 `RuntimeTurnOutcome` -> `ServerNotification` producer 测试。
- [x] provider/runtime 事件模型有真实 mapping；不能从 provider 拿到的事件不 fake。
- [x] capability matrix、schema、supported route、compatibility profile 与实际 producer 一致。
- [x] gap matrix R5 状态只标记已受测完成项，并保留真实 runtime steer 支持的剩余风险说明。
- [x] 验证命令通过，或最终报告明确说明失败命令、失败原因与下一步。

## 11. Commit Message Transparency

实现提交需要包含一行：

```text
已检查 R5 turn steer / plan / diff / raw reasoning producer 是否已有，结论：Codex app-server 有完整参考；Dasclaw 仅已有部分协议词汇，R5 本轮补 app-server route、runtime bridge producer 与 capability/profile 接线。
```

## 12. Follow-up Task 8 - 接入真实 provider R5 producer

**Files:**

- Modify: `crates/dasclaw_llm_provider/src/provider/claw_code_provider.rs`
- Modify: `crates/dasclaw_llm_provider/src/provider/codex_chatgpt.rs`
- Modify as needed: `crates/dasclaw_llm_provider/src/provider/provider.rs`
- Test: provider fixture tests in the same crate, colocated with existing provider/parser tests

- [x] **Step 1: 写真实 provider fixture 测试，而不是 scripted provider 测试**

至少新增以下测试名，并使用 provider/parser 可真实消费的 fixture payload：

```rust
#[test]
fn claw_code_provider_fixture_emits_reasoning_raw_text_delta() {
    // Feed a real claw-code stream chunk fixture that contains raw reasoning delta.
    // Expected: parser emits LlmStreamEvent::ReasoningRawTextDelta.
}

#[test]
fn codex_chatgpt_fixture_emits_raw_response_item_completed() {
    // Feed a real Responses SSE fixture containing response.output_item.done/completed.
    // Expected: parser emits LlmStreamEvent::RawResponseItemCompleted.
}

#[test]
fn provider_fixture_emits_plan_or_diff_only_when_source_event_exists() {
    // Feed a real event source that actually contains plan/diff data.
    // Expected: parser emits TurnPlanUpdated / TurnDiffUpdated only when the upstream event exists.
}
```

Expected before implementation: tests fail because current real provider parsers do not emit the R5 events.

- [x] **Step 2: 实现真实 parser 映射**

只映射 upstream payload 中真实存在的事件；不要从 ordinary text 或 tool call 中推断/伪造 plan/diff/raw item。

Required behavior:

```rust
match upstream_event_name {
    "response.reasoning_text.delta" | "response.reasoning_content.delta" => {
        // emit LlmStreamEvent::ReasoningRawTextDelta
    }
    "response.output_item.done" | "response.output_item.completed" => {
        // emit LlmStreamEvent::RawResponseItemCompleted { item: raw_json }
    }
    "turn.plan.updated" => {
        // emit LlmStreamEvent::TurnPlanUpdated only if plan steps are present
    }
    "turn.diff.updated" => {
        // emit LlmStreamEvent::TurnDiffUpdated only if diff text is present
    }
    _ => {
        // keep existing behavior
    }
}
```

- [x] **Step 3: 保留不可获得事件的 explicit unsupported 语义**

如果 `claw_code_provider` 或 `codex_chatgpt` 的真实 upstream 不提供 plan/diff/raw reasoning/raw item 中的某一类事件：

```rust
// Upstream does not expose turn-level plan/diff events here. Do not synthesize them
// from assistant text; app-server capability remains incomplete for this producer.
```

并在测试中断言 ordinary text 不会被伪造成 `PlanDelta` / `TurnPlanUpdated`。

- [x] **Step 4: 运行 provider 级验证**

```bash
cargo nextest run -p dasclaw_llm_provider -E 'test(provider_fixture_) | test(claw_code_provider_) | test(codex_chatgpt_)'
cargo check -p dasclaw_llm_provider --tests
```

Expected: fixture tests pass; no R5 event is produced from unrelated text/tool chunks.

## 13. Follow-up Task 9 - 收紧 R5 adapter/app-server 测试，区分 synthetic 与真实证据

**Files:**

- Modify: `crates/dasclaw_runtime/src/llm_adapter.rs`
- Modify: `crates/dasclaw_app_server/src/lib.rs`
- Modify: `crates/dasclaw_llm_provider/src/provider/reasoning.rs`
- Test: existing runtime/app-server tests plus new provider-backed integration test where feasible

- [x] **Step 1: 给 scripted tests 改名或注释，明确它们只证明接收路径**

当前 scripted tests 可以保留，但名称或注释必须表达边界：

```rust
#[tokio::test]
async fn scripted_provider_plan_delta_is_forwarded_by_adapter() {
    // This test proves adapter forwarding only. Real provider production is
    // covered by provider fixture tests.
}
```

- [x] **Step 2: 增加 provider-backed integration path**

新增至少一个从真实 provider fixture 到 app-server notification 的测试链：

```rust
#[test]
fn provider_fixture_raw_response_item_reaches_app_server_notification() {
    // fixture -> provider LlmStreamEvent -> AgentEvent -> RuntimeTurnUpdateSink
    // -> ServerNotification::rawResponseItem/completed
}
```

如果直接跨 crate fixture 难以复用，先抽一个 provider parser helper，避免复制 SSE parsing 逻辑。

- [x] **Step 3: 显式处理 `provider/reasoning.rs` 的 R5 events**

不要让新增 R5 events 在 wrapper 中无声吞吐。可接受的实现是：

```rust
LlmStreamEvent::PlanDelta { .. }
| LlmStreamEvent::TurnPlanUpdated { .. }
| LlmStreamEvent::TurnDiffUpdated { .. }
| LlmStreamEvent::RawResponseItemCompleted { .. } => {
    // Deliberately pass through or deliberately ignore with a reason that is
    // asserted by tests. Do not silently drop future R5 producer evidence.
}
```

对应测试要覆盖 wrapper 不会把 R5 events 误标成 reasoning summary，也不会在未声明的情况下丢掉 producer evidence。

- [x] **Step 4: 运行验证**

```bash
cargo nextest run \
  -p dasclaw_runtime \
  -p dasclaw_app_server \
  -E 'test(provider_fixture_) | test(scripted_provider_) | test(dasclaw_runtime_bridge_forwards_r5_) | test(r5_runtime_updates_)'
cargo check -p dasclaw_runtime -p dasclaw_app_server --tests
```

Expected: scripted tests 继续证明 forwarding；provider-backed tests 证明至少一个真实 producer path。

## 14. Follow-up Task 10 - 实现或继续显式保留真实 `turn/steer` 缺口

**Files:**

- Modify: `crates/dasclaw_app_server/src/lib.rs`
- Modify as needed: `crates/dasclaw_runtime/src/*`
- Test: `crates/dasclaw_app_server/src/lib.rs`

- [x] **Step 1: 查找真实运行中追加用户输入 API**

用 `rg` / LSP 查找 runtime 是否已有 running turn input injection：

```bash
rg -n "steer|inject|append.*input|running.*turn|active.*turn|user.*input" crates/dasclaw_runtime crates/dasclaw_core crates/dasclaw_app_server
```

Expected:

- If a real API exists, implement `DasclawAgentRuntimeBridge::steer_turn` against it.
- If no real API exists, keep `turn_steer: false` for `DasclawAgentRuntimeBridge` and do not strike `turn/steer` in gap matrix.

- [ ] **Step 2A: 如果真实 API 存在，补真实 steer test**

```rust
#[test]
fn dasclaw_runtime_bridge_steer_turn_injects_input_into_running_turn() {
    // Start a real pending turn with a controllable runtime.
    // Call turn/steer with expectedTurnId.
    // Assert the running turn receives the additional UserInput exactly once.
}
```

- [x] **Step 2B: 如果真实 API 不存在，补文档与测试锁定 fail-safe**

```rust
#[test]
fn default_dasclaw_runtime_bridge_does_not_advertise_turn_steer_until_real_injection_exists() {
    let bridge = DasclawAgentRuntimeBridge::from_model_provider_snapshot();
    assert!(!bridge.features().turn_steer);
}
```

- [x] **Step 3: gap matrix 只按真实状态更新**

Rules:

- `turn/steer` 只有在 Step 2A 通过后才能加删除线。
- 如果只完成 Step 2B，gap matrix 必须写成 `route/fail-safe done, real runtime injection pending`。
- 不允许用 `RecordingRuntimeBridge::with_turn_steer()` 的测试作为真实 steer 完成证据。

## 15. Follow-up Task 11 - 重新更新 gap matrix

**Files:**

- Modify: `docs/plans/dasclaw-app-server-codex-protocol-gap-matrix.md`

- [x] **Step 1: 拆分 R5 状态，不整体划掉**

将 R5 拆成可独立验收的子项：

| 子项 | 状态规则 |
|---|---|
| R5a protocol/router/adapter receive path | 可以标完成，如果 schema、route、adapter forwarding、app-server notification tests 通过 |
| R5b real provider producer | 只有真实 provider fixture/parser tests 通过后才能标完成 |
| R5c real turn steer injection | 只有真实 `DasclawAgentRuntimeBridge::steer_turn` 注入 running turn 并受测后才能标完成 |

- [x] **Step 2: 写入复审边界**

gap matrix 必须包含这句等价信息：

```markdown
当前 R5 不得整体划掉：已完成 protocol/router/adapter receive path；真实 provider producer 与真实 turn steer injection 仍未完成，scripted provider / synthetic bridge 测试不能作为完成证据。
```

- [x] **Step 3: 运行文档核验**

```bash
rg -n "R5|turn/steer|scripted|synthetic|真实 provider|provider producer|turn_steer" docs/plans/dasclaw-app-server-codex-protocol-gap-matrix.md
git diff --check
```

Expected: R5 不再被整体删除线误标；未完成项有明确后续 owner。
