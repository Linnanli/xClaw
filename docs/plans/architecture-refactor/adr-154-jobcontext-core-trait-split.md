# ADR-154（草稿）：把 `JobContext` 上帝结构体拆成 `JobContextCore` trait + ironclaw GUI 扩展

- 状态：Draft（讨论中，尚未拍板）
- 提案人：Coding Agent（受用户授权，基于 #672 阻塞分析）
- 关联：ADR-152（agent and capability fusion）、ADR-153 草稿（无头 agent 框架）、ADR-129（verbatim port mandate）
- 关联 issue：#672（解 `JobContext` / `WebhookCapability` 耦合后把 `Tool` trait + `LspQueryTool` 搬到 `dasclaw_tool`）
- 时间：2026-05-20

---

## 1. 背景

ADR-152 §3 F3.3 要求把 `Tool` trait 与所有内置工具搬到 `dasclaw_tool`，让任何 dasclaw 宿主（桌面、CLI、headless、admin-backend）都能注册同一套工具。
切片 1/2（PR #671）已经把工具层的纯数据类型与参数 helper（`ApprovalRequirement`、`RiskLevel`、`ToolOutput`、`ToolSchema`、`redact_params` 等）搬走，**剩下的 `Tool` trait 与 `LspQueryTool` 卡在 #672**。

卡住的原因是一个签名：

```rust
async fn execute(&self, params: &Map<String, Value>, ctx: &JobContext) -> Result<ToolOutput, ToolError>;
```

`JobContext` 住在 `desktop-client/ironclaw/src/context/state.rs`，它的源码注释（第 6–9 行）自己已经写了：

> This file ... keeps the **ironclaw-specific `JobContext` god-struct, which still owns GUI/marketplace fields**.
> The god-struct split into a `JobContextCore` trait + GUI extension is the follow-up sub-PR.

也就是说，这个拆分本来就在路线图上，只是一直没人动。本 ADR 就是把这件事正式落下来。

## 2. 现状盘点：`JobContext` 到底装了什么

`JobContext` 当前 27 个字段，按用途分三类。

### 2.1 工具真正在用的字段（来自全仓 grep `ctx\.(...)`）

| 字段 | 实际读 / 写 | 出现位置（举例） |
|---|---|---|
| `user_id` | 读，高频 | `memory.rs`、`secrets_tools.rs`、`shell.rs`、`http.rs`、`job.rs` |
| `conversation_id` | 读 + 写 | `sub_agent.rs:270`、`session_fork.rs:119`、`plan_mode.rs` |
| `state` | 读 | `job.rs:1039-1041` |
| `metadata` | 读 + 写 | `path_utils.rs`、`shell.rs`、`message.rs`、`job.rs:1886`、`lsp/tool.rs:533`、`time.rs` |
| `tool_output_stash` | 读 + 写 | `json.rs` |
| `user_timezone` | 读 | `time.rs`、`memory.rs` |
| `extra_env` | 读 | `shell.rs:974` |
| `http_interceptor` | 读 | `http.rs:599, 808` |
| `feature_flags` | 工具内未直接读，但 agent loop 用 | `tools/feature_flags.rs` |
| `title`, `description` | 读（仅 `job.rs` 内部跨任务摘要用） | `job.rs:1048, 1132` |

### 2.2 GUI / marketplace 字段 — 工具**从不读写**

经全仓 grep 确认，**没有任何 `Tool::execute` 的实现读以下字段**：

```
budget, budget_token, bid_amount, estimated_cost, estimated_duration,
actual_cost, total_tokens_used, max_tokens, repair_attempts,
created_at, started_at, completed_at, transitions, requester_id
```

它们仅在桌面前端的看板 / 估价 / 计费 / 历史回放路径里被读。

### 2.3 中间字段（部分耦合）

- `transitions` — 状态机转换历史，由 agent loop 写，工具不读。
- `requester_id` — 多通道场景（如 Telegram bot 接到的 chat 里识别真实用户），工具不读。

## 3. 决策

### 3.1 抽 trait `JobContextCore`，放到 `dasclaw_runtime`

> **落点选 `dasclaw_runtime` 而非 `dasclaw_tool`**：trait 里包含 `JobState`、`Uuid`、`HttpInterceptor` 等运行时类型，`dasclaw_tool` 应保持依赖轻。`dasclaw_tool` 已经依赖 `dasclaw_runtime`（见 PR #641 把 `JobState` 搬过去那一步），所以 `Tool::execute(ctx: &dyn JobContextCore)` 在 `dasclaw_tool` 里能直接引用。

trait 最小接口（基于上面盘点的 §2.1）：

```rust
// crates/dasclaw_runtime/src/job_context.rs（新建）
pub trait JobContextCore: Send + Sync {
    // —— 标识 ——
    fn job_id(&self) -> Uuid;
    fn user_id(&self) -> &str;
    fn requester_id(&self) -> Option<&str>;
    fn conversation_id(&self) -> Option<Uuid>;
    fn set_conversation_id(&mut self, id: Option<Uuid>);

    // —— 状态 ——
    fn state(&self) -> JobState;

    // —— 工具读写共享数据 ——
    fn metadata(&self) -> &serde_json::Value;
    fn set_metadata(&mut self, value: serde_json::Value);
    fn tool_output_stash(&self) -> Arc<tokio::sync::RwLock<HashMap<String, String>>>;
    fn extra_env(&self) -> Arc<HashMap<String, String>>;

    // —— 环境 / 录制 ——
    fn user_timezone(&self) -> &str;
    fn http_interceptor(&self) -> Option<Arc<dyn HttpInterceptor>>;

    // —— 能力门 ——
    fn feature_flags(&self) -> SharedFeatureFlags;

    // —— 摘要（job.rs 跨任务列表用） ——
    fn title(&self) -> &str;
    fn description(&self) -> &str;
}
```

其中：

- `HttpInterceptor` trait 与 `SharedFeatureFlags` 需要先随 trait 一起搬到 `dasclaw_runtime`（属于本 ADR 的实现 PR 范围，但搬的是 trait 与共享别名，**不是** ironclaw 业务实现，符合 verbatim 原则）。
- `JobState`、`StateTransition`、`TokenBudgetExceeded` 早在 #641 已搬到 `dasclaw_runtime::job`，无须再动。

### 3.2 `ironclaw::JobContext` 保留 + 实现 trait

```rust
// desktop-client/ironclaw/src/context/state.rs（保持文件名不动）
pub struct JobContext {
    /* 27 个字段保持原样，**不删任何字段** */
}

impl JobContextCore for JobContext {
    fn job_id(&self) -> Uuid { self.job_id }
    fn user_id(&self) -> &str { &self.user_id }
    /* ...全部 13 个方法，每个一行，零业务逻辑... */
}
```

- GUI / marketplace 字段（`budget`、`bid_amount`、`actual_cost`、`max_tokens`、`transitions`...）继续住在这里，**不进 trait**，桌面后端继续按字段访问，零变化。
- 既然字段不删、构造函数 `JobContext::new()` / `JobContext::with_user()` 也不动，对 ironclaw 内部所有「以 `JobContext` 为入参或字段」的代码而言**公共 API 表面零回归**。

### 3.3 `Tool::execute` 签名改成 `&dyn JobContextCore`

```rust
// crates/dasclaw_tool/src/lib.rs（搬迁 + 改签名）
#[async_trait]
pub trait Tool: Send + Sync {
    async fn execute(
        &self,
        params: &Map<String, Value>,
        ctx: &mut dyn JobContextCore,  // 由 &JobContext 改为 &mut dyn JobContextCore
    ) -> Result<ToolOutput, ToolError>;
    /* 其余默认方法与 #671 已搬走的辅助类型保持不变 */
}
```

> 为什么用 `&mut dyn` 而非 `&dyn`：§2.1 里 `sub_agent.rs` / `session_fork.rs` / `job.rs` 确实写 `ctx.conversation_id`、`ctx.metadata`。把这些通过 trait setter 暴露，比让工具持有 `Arc<Mutex<...>>` 干净。

### 3.4 调用点改造（机械替换）

ironclaw 调用 `tool.execute(&params, &ctx)` 的所有位置（grep 估计 < 20 处），改成 `tool.execute(&params, &mut ctx as &mut dyn JobContextCore)`。其他工具实现内部 `ctx.user_id` 改成 `ctx.user_id()`。

> 这是机械替换，不是设计变更。**关键**：本 ADR 的实现 PR 只允许这种机械替换，不允许借机重构任何工具的内部逻辑（verbatim 原则的延伸）。

### 3.5 `WebhookCapability` 同步迁移

`Tool::webhook_capability()` 当前返回 `Option<crate::tools::wasm::WebhookCapability>`。
`WebhookCapability` 是 7 字段全 `Option<String>` 的纯数据 struct（已确认零 ironclaw 依赖），随本 ADR 实现 PR 一并 verbatim 搬到 `dasclaw_tool`，ironclaw 侧 `pub use` 薄壳保留。

## 4. 不做什么（Non-goals）

1. **不**改 `JobContext` 的字段集合（含 GUI/marketplace 字段全部保留）。
2. **不**改 `JobContext::new()` / `JobContext::with_user()` 与所有 builder 风格构造方法。
3. **不**改任何工具的业务逻辑——只把字段访问 `ctx.user_id` 改成方法调用 `ctx.user_id()`。
4. **不**在本 ADR 实现 PR 里搬 `LspQueryTool`（留给 #672 的下一切片）。
5. **不**引入新的依赖（trait 用的 `Uuid`、`Arc`、`tokio::sync::RwLock`、`serde_json::Value` 都是已有的）。
6. **不**改 `HookRegistry` 与 hook 唯一入口（ADR-113 不变）。

## 5. 验收基线（实现 PR 必须满足）

- [ ] `cargo nextest run -p dasclaw_runtime` 含新增 trait 默认 / 边界用例。
- [ ] `cargo nextest run -p dasclaw_tool` 含 `Tool` trait 的 `EchoTool` 等回归。
- [ ] `cargo check -p dasclaw --tests` 编译通过。
- [ ] `cargo nextest run -p dasclaw` 全量回归对照旧基线**通过数不下降**。
- [ ] `JobContext` 字段集合通过 `grep -c "pub " desktop-client/ironclaw/src/context/state.rs` 校验**与本 ADR 落地前一致**。
- [ ] `python3.12 scripts/check_no_panics.py --base origin/xClaw` 通过。
- [ ] `python3 scripts/check_no_new_ironclaw_literal.py --base origin/xClaw` 通过。
- [ ] 公共 API 表面：`ironclaw::tools::tool` 路径下原有 `pub use ...` 不删不改。

## 6. 与 ADR-153（无头框架）的关系

ADR-153 的目标形态是：

```rust
let agent = Agent::builder()
    .llm(llm)
    .workspace(...)
    .tools_default()
    .hooks_default()
    .build()?;
```

那个 `tools_default()` 注册的工具会调用 `tool.execute(&params, ctx)`。如果 `ctx` 是 `ironclaw::JobContext`，调用方就被迫拖进整套桌面客户端依赖；如果 `ctx` 是 `dyn JobContextCore`，调用方只需要一个轻量实现（甚至可以是 `struct MinimalCtx { user_id: String, ... }`）。

**本 ADR 是 ADR-153「无头框架」能不能跑通的真正前提**。在 trait 抽离前，「无头 agent」最多只是个口号——任何使用 Tool 的代码都会自动跟桌面客户端 7 万行 ironclaw 业务绑死。

## 7. 风险与回滚

| 风险 | 缓解 |
|---|---|
| trait 方法签名与现有工具的字段访问匹配错误 | 实现 PR 拆成两个 commit：① 加 trait + impl JobContext，零调用点改动；② 改 `Tool::execute` 签名，机械替换所有调用点。每步独立 `cargo check` 与 `cargo nextest`。 |
| `&mut dyn` 与现有 `async_trait` 组合在 lifetime 上出毛病 | trait 用 `Send + Sync` 约束；如 `async_trait` 不接受，回退到 `&self, ctx: &mut Box<dyn JobContextCore + '_>`。 |
| 工具直接读 `ctx.metadata` 用了 serde_json 的 borrow API | trait 暴露 `&serde_json::Value`（read）与 `set_metadata(Value)`（write），与现有用法对齐。 |
| 性能回归（动态分发） | `Tool::execute` 已经是动态分发（`Box<dyn Tool>`），多一层 trait object 量级一致；agent loop 每 tool call 调用 N 次，N ≤ 千级别，无热路径风险。 |

如果实现 PR 在 `cargo nextest` 全量回归比基线少任何 1 条用例，**整 PR 回滚**，不允许 patch 修补。

## 8. 落地路径

1. **本 ADR 落地**（即本 PR）：仅添加 `docs/plans/architecture-refactor/adr-154-...md`，state=Draft。
2. **ADR Accepted**：与 ADR-153 草稿一起在下一次架构对齐会被裁定。如果 Accept，状态从 Draft 改为 Accepted。
3. **实现 PR**（独立分支，独立 PR，独立 review）：按 §3 + §7 双 commit 落地。
4. **#672 第二切片**：实现 PR 合入后，搬 `Tool` trait + `LspQueryTool` 到 `dasclaw_tool` / `dasclaw_lsp`。

## 9. 参考

- ADR-152 §3 F3.3（tool 层拆分总目标）
- ADR-153 草稿（无头 agent 框架）
- ADR-129 §1.3（verbatim port 基线 — 本 ADR 的所有实现步骤都遵守）
- ADR-113（HookEngine 唯一入口 — 本 ADR 不动）
- PR #641（`JobState` 搬到 `dasclaw_runtime` 的先例）
- PR #671（#670 切片 1/2：tool 辅助类型与 helper 已搬走）
- issue #672（本 ADR 要解锁的工作）
- `desktop-client/ironclaw/src/context/state.rs` 第 6–9 行（god-struct 拆分预告）
