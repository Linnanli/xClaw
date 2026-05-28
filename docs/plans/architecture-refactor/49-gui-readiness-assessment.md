# x-claw 无头 agent 框架 GUI 接入度评估

> **状态**：调研报告（2026-05-27 v1，2026-05-27 v1.1 二次验证）
> **后续 issue**：#907 (B1) / #908 (B2) / #909 (B3) / #910 (B4) / #911 (B5)
> **关联 ADR**：ADR-152、ADR-153、ADR-154
> **前序文档**：[48-agent-framework-readiness-and-task-audit.md](./48-agent-framework-readiness-and-task-audit.md)

## v1.1 验证戳（2026-05-27）

报告 v1 完成后，重建了 code-review-graph（xclaw-core 6871 节点 / 45882 边、新增 desktop-client-app + desktop-client-ui 分库、Leiden 社区检测启用），并对 5 个 blocker 重新做了行号级 grep 二次验证：

| Blocker | 行号证据 | 论断状态 |
|---|---|---|
| B1 cancel | [`crates/dasclaw_runtime/src/agent.rs#L382-L384`](../../../crates/dasclaw_runtime/src/agent.rs) — `HeadlessDelegate::check_signals` 实现就 `LoopSignal::Continue` 一行硬编码 | ✅ 不变 |
| B2 streaming | [`crates/dasclaw_core/src/llm.rs#L137`](../../../crates/dasclaw_core/src/llm.rs) + [`circuit_breaker.rs#L325`](../../../crates/dasclaw_llm_provider/src/provider/circuit_breaker.rs) + [`retry.rs#L280`](../../../crates/dasclaw_llm_provider/src/provider/retry.rs) 都透传 `chunk_tx`，但 `dasclaw_runtime/src/agent.rs` 里搜不到任何 `chunk_tx` —— `AgentResponder` trait 没把流通道暴露给 Agent | ✅ 不变 |
| B3 serde | 7 个公开类型（`RespondOutput`/`RespondResult`/`ToolResult`/`FinishReason`/`AgentError`/`LoopOutcome`/`HostError`）正则 `#[derive(...Serialize...)] pub (enum\|struct) <type>` **零匹配** | ✅ 不变 |
| B4 approval | `ApprovalRequested` 全 crates 树仅 2 处：[`agent.rs#L163`](../../../crates/dasclaw_runtime/src/agent.rs) 错误变体定义 + [`agent.rs#L524`](../../../crates/dasclaw_runtime/src/agent.rs) 从 `LoopOutcome::NeedApproval(_)` 映射成 error；零 event/broadcast 入口 | ✅ 不变 |
| B5 claw-code | `claw-code/**/*.rs` 里 `dasclaw_` 零匹配 | ✅ 不变 |

5 个 blocker 全部依然成立，推荐实施顺序 [§9](#9-推荐实施顺序与里程碑) 不变。

## TL;DR（≈ 65% 就绪）

- 公共面 API 已经成型：`dasclaw_runtime::Agent::builder()` + 三个 narrow trait seam（responder / tool / sanitizer）+ `HookBundle` 是稳定接入面，`crates/dasclaw_cli` 是官方嵌入参考实现，证明"无 Tauri / 无 DB / 无 HTTP" 单进程跑通。
- **依赖隔离干净**：`crates/dasclaw_*` 全仓零 `tauri / iced / egui / wry / webview` 引用，GUI 框架可自由选型。
- **三个 P0 阻塞**让 GUI 还不能"开箱即用"：① `Agent::run` 没有 cancel handle / Session 句柄（#907）；② `Agent` 这一层不暴露 token 级 streaming（#908）；③ 关键中间类型（`RespondOutput` / `ToolResult` / `AgentError` / `FinishReason`）缺 `Serialize`，IPC 桥要手写（#909）。
- claw-code 与 dasclaw 复用栈尚未对接（grep 零命中），三库融合 contract 在 GUI 路径上未端到端证明（与 [48-md §0](./48-agent-framework-readiness-and-task-audit.md#0-结论) 一致）。

## 1. 接入面 API 盘点

唯一对外入口是 [`crates/dasclaw_runtime/src/agent.rs`](../../crates/dasclaw_runtime/src/agent.rs)（ADR-153 §4.2 落点）：

- `Agent::builder()` → `AgentBuilder` 提供 `.responder() / .tool_executor() / .tool_output_sanitizer() / .tools() / .hooks() / .system_prompt() / .model() / .loop_config() / .build()`，全部 `#[must_use]`，无 patch-style 可选参数。
- 三个 narrow trait seam：`AgentResponder`（LLM 适配）、`ToolExecutor`（工具调度）、`ToolOutputSanitizer`（脱敏）。`Arc<dyn ToolExecutor>` 自动 blanket impl，便于动态拼装。
- `Agent::run(&str) -> Result<String, AgentError>` —— 单次请求 / 响应。
- 现成桥接：`LlmProviderResponder` 把任意 `dasclaw_llm_provider::LlmProvider` 适配成 `AgentResponder`；`dasclaw_cli::tools::StaticToolExecutor`、`McpToolExecutor` 提供工具维度的现成实现。
- `JobContextCore` trait 已抽到 `dasclaw_runtime`（ADR-154），GUI 宿主只需实现 10 来个方法即可注入 Job 上下文。

## 2. 数据契约（Serde）覆盖率

- ✅ 有 `Serialize + Deserialize`：`ChatMessage`、`ToolCall`、`ToolDefinition`、`JobState`、`StateTransition`、`HttpExchange*`、`Secret*`。
- ❌ 缺 serde：`RespondOutput` / `RespondResult` / `ToolResult` / `FinishReason` / `AgentError` / `LoopOutcome`，全部只有 `Debug + Clone`。
- 影响：Tauri `invoke()` / Electron IPC / WebSocket 想把"模型这一轮的产物""工具调用结果""失败原因"原样推到前端，**都必须先在宿主里手写转换层**。
- **行动项**：issue #909（最低风险 P0）

## 3. 生命周期管理

- ✅ `JobState` 五态状态机（Pending / InProgress / Stuck / Accepted / Failed / Cancelled）+ 合法转换断言，`JobContextCore::transition_to(...)` 是受控入口。
- ✅ `LoopDelegate::check_signals -> LoopSignal::{Continue, Stop, InjectMessage}` 是底层 cancel 钩子。
- ❌ 但 `Agent` 内部 `HeadlessDelegate::check_signals`（`crates/dasclaw_runtime/src/agent.rs:407`）写死返回 `Continue`，**`AgentBuilder` 也没有 `.cancellation_token(...)` / `.signal_rx(...)`**。GUI 上的"停止"按钮目前没有原生通道，只能调用方自己包一层 `AgentResponder` 实现 cancel 检查。
- ❌ 没有 `Session` / `Conversation` 句柄：每次 `Agent::run` 都新建 `ReasoningContext`，多轮对话状态需要调用方保存 `Vec<ChatMessage>` 然后自己塞回。`dasclaw_session` 在 ADR-153 §4.3 标记为"可选"，**尚未落地**。
- **行动项**：issue #907

### "没暴露" vs "功能没做" 区分

- **cancel/stop**：底层做了（LoopDelegate signal 机制），门面没开 —— 修复就是加 `AgentBuilder::cancellation_token()` 配置点
- **Session 句柄**：真功能缺失 —— 需要新增 `Session` 类型，持有 `ReasoningContext` 续接历史

## 4. 流式输出

- 底层有：`dasclaw_llm_provider::respond_with_tools_streaming`（`crates/dasclaw_llm_provider/src/provider/reasoning.rs:626`）接 `tokio::sync::mpsc::UnboundedSender<String>` chunk 通道。
- 接入面没暴露：`AgentResponder::respond` 只返回完整 `RespondOutput`；`Agent::run` 等到 `LoopOutcome::Response(String)` 才回；没有 `Agent::run_streaming(...)` 或类似 `Stream<Item = AgentEvent>` 接口。
- GUI 想做 typewriter 必须自己实现 `AgentResponder`，在内部调用 streaming provider 并把 chunks 通过自己的 channel 转发出来。可行但属于"绕路"，不是 first-class。
- **行动项**：issue #908

## 5. 错误传播

- `AgentError` 用 `thiserror::Error` 分了 7 个变体（MissingResponder / MaxIterations / ToolsNotSupported / LoopFailure / Stopped / ApprovalRequested / Responder），文案明确。
- 底层 `HostError = Box<dyn std::error::Error + Send + Sync>`（`crates/dasclaw_core/src/traits.rs:41`）是类型擦除，IPC 端只能拿到 `to_string()`。
- `ApprovalRequested` 在 headless 模式只能 fail-stop，**没有"前端审批 → 继续"原生回路**。
- **行动项**：issue #910（依赖 #908）

## 6. 依赖隔离验证

- `grep_search`（regex）扫 `crates/dasclaw_*/Cargo.toml` 全部 `tauri|iced|egui|wry|webview|tray-icon`：**零命中**。
- `dasclaw_runtime` 真实依赖：`dasclaw_tool / dasclaw_core / dasclaw_llm_provider + tokio(sync) + serde + uuid + chrono + secrecy + aes-gcm + thiserror`，没有 GUI / HTTP server / DB 客户端必选项（postgres / libsql 是 optional feature）。
- 平台 keychain 由 `cfg(target_os = ...)` gate，跨平台 GUI 不会被强迫拉。
- 结论：依赖面**真的 headless**，GUI 端选 Tauri / Electron / iced 任一都不会引入冲突。

## 7. 现有客户端样板代码

| 宿主 | 路径 | 接入方式 | 评价 |
|---|---|---|---|
| `dasclaw_cli` | `crates/dasclaw_cli/src/lib.rs` + `main.rs` | `Agent::builder()` + `StaticToolExecutor` + `McpToolExecutor` + `SafetyStashSanitizer` | 官方 reference，PR #800/#802/#804/#806/#812/#814/#817 闭环 |
| `desktop-client/ironclaw` | `src/agent/dispatcher.rs`（`ChatDelegate`） | **绕过 `Agent` facade**，直接实现自己的 `LoopDelegate` 调 `dasclaw_core::agentic_loop::run_agentic_loop` | 多租户 / 审批 / 历史 / channels 定制深度超出 `Agent` 设计目标 |
| `claw-code/` | — | `grep -r dasclaw claw-code/` **零命中** | 三库融合在 GUI 路径上未端到端证明 |

`dasclaw_cli` 是 GUI 接入直接可抄的最小样板；ironclaw 路径告诉你"如果定制深度足够大，应该实现自己的 `LoopDelegate` 而不是用 `Agent`"。

## 8. 阻塞清单（Top 5）

| 编号 | issue | 主题 | 优先级 |
|---|---|---|---|
| B1 | #907 | Cancel / Session 句柄缺失 | P0 |
| B2 | #908 | `Agent` 层无 streaming 入口 | P0（依赖 B3） |
| B3 | #909 | IPC 数据契约不完整（6 类型缺 serde） | **P0 最低风险** |
| B4 | #910 | 前端审批回路缺失 | P1（依赖 B2） |
| B5 | #911 | claw-code 未接入 + 三库融合未在 GUI 证明 | P1（依赖 B1/B2/B3） |

## 9. 推荐路径

按依赖顺序、单 PR 可 revert：

1. **第一刀（P0，解 B3 #909）**：给 `RespondOutput / RespondResult / ToolResult / FinishReason / LoopOutcome / AgentError` 加 `#[derive(Serialize, Deserialize)]`（`AgentError` 用 `#[serde(tag = "kind")]` 平坦化）。零运行时风险。
2. **第二刀（P0，解 B1 #907）**：`AgentBuilder::cancellation_token(CancellationToken)` + 内部 `HeadlessDelegate::check_signals` 检查 token。同步落 `Agent::session()` 返回 `Session` 句柄持有 `ReasoningContext`，提供 `Session::run(prompt)`；旧 `Agent::run` 委托给一次性 session。
3. **第三刀（P0，解 B2 #908）**：新增 `AgentResponder::respond_streaming(ctx, chunk_tx)` 默认 impl 退化到 `respond`；`Agent` 增 `run_streaming(prompt, event_tx: mpsc<AgentEvent>) -> Result<String, AgentError>`，事件枚举 `{TextChunk, ToolCallStart, ToolResult, FinishReason}` 全 serde。
4. **第四刀（P1，解 B4 #910）**：把 `ApprovalRequested` 从错误变体拆到 `AgentEvent::ApprovalNeeded { … resume_tx: oneshot<Decision> }`。
5. **第五刀（P1，解 B5 #911）**：在 `dasclaw_cli` 加 `claw-code` 互操作 demo（最小 governance hook + bash validator），验证三库融合的 GUI 路径。

按这个序列推进，GUI 接入度可在 4–5 个串行 PR 内推到 ~90%；当前 65% 的核心瓶颈在 B1/B2/B3，而非架构方向。

## 调研工具足迹

- `read_file`：ADR-152 / ADR-153 / ADR-154 / 48-md；`dasclaw_runtime/src/{lib,agent,tool,job_context}.rs`；`dasclaw_core/src/{messages,response_types,agentic_loop,traits}.rs`；`dasclaw_cli/src/{lib,main}.rs`；`dasclaw_runtime/Cargo.toml`。
- `grep_search`（regex，三层验证否定性结论）：`CancellationToken|mpsc::|Stream`、`#[derive(...Serialize`、`tauri|iced|egui|wry|webview`、`dasclaw_runtime::Agent`、`dasclaw`（在 `claw-code/` 下零命中）。
- `list_dir`：`docs/plans/architecture-refactor/`、`crates/dasclaw_runtime/src/`。
- `file_search`：定位 `lib.rs / main.rs / agent.rs` 入口。
- **未跑（偏差说明）**：`cargo check / nextest`、code-review-graph MCP。后续 B1/B2 实施前建议补一次 graph build 限定 `crates/dasclaw_*` + `desktop-client/ironclaw/src/`，借枢纽节点 / 桥接节点 / 影响半径视角避免漏改 ironclaw 侧调用点。
