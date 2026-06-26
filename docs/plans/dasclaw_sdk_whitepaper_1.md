我想把这个改成一个agent sdk, 当前的架构和代码是否符合这个定位, 应该怎么调整

## 结论

**架构方向符合 Agent SDK，但当前代码形态仍更像“某个具体 Agent 应用拆出来的内部框架”，还不是可稳定发布的通用 SDK。**

我的判断：

* 作为内部无头 Agent 框架：**7/10**
* 作为第三方可嵌入、可扩展、长期兼容的 Rust Agent SDK：**4.5/10**

不需要推翻重写。模型适配、Agent 循环、工具、MCP、Session、沙箱等实现大多可以保留；需要重点重构的是：

> **公共 API 边界、Agent 与单次运行的关系、工具扩展协议、依赖方向和发布结构。**

---

# 一、当前哪些部分符合 SDK 定位

现在已经有不少 SDK 所需的核心积累：

| 能力       | 当前基础                                        |
| -------- | ------------------------------------------- |
| 无头运行     | 已有 `Agent`、Builder、invoke、stream            |
| Agent 循环 | 已有多轮推理、工具调用、最大轮次、取消                         |
| 模型适配     | 已有多 Provider、流式响应、模型切换、重试和故障转移              |
| 工具扩展     | 已有 Tool、ToolExecutor、MCP、WASM 和大量内置工具       |
| 人工介入     | 已有 Approval、信号注入、取消等机制                      |
| Session  | 已有 Snapshot、Store 和会话恢复方向                   |
| 安全扩展     | 已有沙箱、Egress Gate、SecretProvider、Sanitizer   |
| 服务化能力    | 已有 App Server、Protocol、CLI，可作为 SDK 的参考 Host |

成熟 Agent SDK 通常会把 Agent 定义、Runner、工具、Guardrail、Session、Tracing 分开。OpenAI 官方 Agents SDK 也是由 Agent 描述配置，再由 Runner 负责循环、工具调用、Guardrail、handoff 和会话运行；这说明你当前的大方向是对的。([OpenAI GitHub][1])

---

# 二、为什么当前还不适合作为通用 SDK

## 1. `Agent` 同时承担了“定义”和“运行实例”

现在 `dasclaw_runtime/src/agent.rs` 中的 `Agent` 不只是 Agent 配置，还持有：

* `signal_tx/rx`
* cancellation token
* approval inbox
* tool executor
* sanitizer
* hooks
* 运行状态相关控制对象

这会产生一个严重问题：**同一个 Agent 并发执行多次时，状态归属不明确。**

例如：

* 一个 steering message 可能被另一个 run 消费；
* cancel 可能取消整个 Agent 的所有运行；
* approval response 不容易严格对应某一次工具调用；
* Agent 无法安全作为全局单例复用。

### 应调整为

```text
Agent
  只描述“它是谁、有什么能力”

Runner
  管理模型、存储、策略、Tracing、执行器

Run
  表示一次具体执行

RunHandle
  管理该次执行的 cancel / steer / approve / events
```

推荐模型：

```rust
let agent = Agent::builder("office-assistant")
    .instructions("你是办公助手")
    .model(model)
    .tool(search_document)
    .build()?;

let handle = runner.start(
    &agent,
    Input::text("查询采购制度"),
    app_context,
).await?;

let result = handle.result().await?;
```

`Agent` 应当是不可变、可复用、`Send + Sync` 的定义对象。所有可变状态都必须下沉到 `RunState`。

---

## 2. `dasclaw_runtime` 依赖过重，不是真正的 SDK Runtime

目前 `dasclaw_runtime/Cargo.toml` 直接或间接包含：

* 具体的 `dasclaw_llm_provider`
* 操作系统 Keychain
* PostgreSQL/libSQL
* 密钥和加密能力
* 产品型 JobContext
* 多种应用层设施

这会导致使用者即使只想实现一个简单 Agent，也可能被迫引入：

* Provider 具体实现；
* 数据库；
* 系统密钥库；
* OS 平台依赖；
* 不相关的产品上下文。

SDK 的核心依赖应该非常轻。

推荐依赖方向：

```text
dasclaw-core
      ↑
dasclaw-runtime
      ↑
dasclaw（Facade）

外部集成：
provider-openai ───→ core/runtime
provider-anthropic → core/runtime
session-postgres ──→ runtime
tool-shell ────────→ core/runtime
mcp ───────────────→ core/runtime
otel ──────────────→ runtime
```

**Runtime 不能反向依赖具体 Provider、数据库、Keychain 或 App Server。**

---

## 3. Tool API 被拆成了多套概念

当前至少存在这些相关类型：

* `dasclaw_core::messages::ToolDefinition`
* `dasclaw_core::messages::ToolCall`
* `dasclaw_tool::ToolSchema`
* `dasclaw_runtime::Tool`
* `dasclaw_runtime::ToolExecutor`
* `dasclaw_core::agentic_loop::ToolDispatcher`
* `CompositeToolExecutor`

这对内部迁移代码尚可接受，但对 SDK 用户会非常困惑：

> “我到底应该实现 Tool、ToolExecutor，还是 ToolDispatcher？”

更大的问题是：

```rust
AgentBuilder::tools(Vec<ToolDefinition>)
AgentBuilder::tool_executor(...)
```

工具描述和工具实现是分别注册的，可能出现：

* 模型看到一个工具，但执行器没有；
* 执行器有一个工具，但没有暴露给模型；
* Schema 更新了，执行实现没有同步；
* Tool 名称在多个位置重复声明。

### 应统一成一个 `ToolRegistry`

```rust
#[async_trait]
pub trait Tool<C>: Send + Sync {
    type Args: DeserializeOwned + JsonSchema + Send;
    type Output: Serialize + Send;

    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;

    async fn call(
        &self,
        ctx: ToolContext<'_, C>,
        args: Self::Args,
    ) -> Result<Self::Output, ToolError>;
}
```

注册时同时注册 Schema 和实现：

```rust
let tools = ToolRegistry::new()
    .register(SearchDocumentTool)
    .register(CreateMeetingTool);
```

内部再通过类型擦除转换为 `DynTool`。

这样可以保证：

```text
工具名称
参数 Schema
输出 Schema
权限信息
审批信息
实际执行代码
```

始终属于同一个对象。

---

## 4. Tool 的扩展接口依赖了过于具体的 `JobContextCore`

当前 `Tool` 接口依赖 `JobContextCore`，而该上下文包含大量产品特定字段：

* job
* requester
* conversation
* title/description
* tool stash
* HTTP interceptor
* feature flags
* metadata
* environment

这相当于要求每个第三方 Tool 都理解你原来应用的 Job 模型。

### 应改成泛型应用上下文

```rust
pub struct ToolContext<'a, C> {
    pub run: &'a RunInfo,
    pub app: &'a C,
    pub services: &'a RuntimeServices,
}
```

用户可自行定义：

```rust
struct OfficeContext {
    tenant_id: String,
    user_id: String,
    department_id: String,
    permissions: Vec<String>,
}
```

然后：

```rust
Agent::<OfficeContext>::builder(...)
```

SDK 可以另外提供可选的政企扩展：

```rust
EnterpriseContext
IdentityContext
DataClassification
DelegationContext
```

但不能把政企字段直接硬编码进通用 Core。

---

## 5. 模型接口过于臃肿

当前 `LlmProvider` 同时负责：

* 模型调用；
* 流式调用；
* 模型列表；
* 模型切换；
* Token 价格；
* Cache 价格；
* 活跃模型；
* Provider 配置。

其中 `set_model(&self)` 这类可变行为，不适合作为并发 SDK 的基础模型接口。

### 建议拆成

```rust
#[async_trait]
pub trait Model: Send + Sync {
    fn id(&self) -> ModelId;

    fn capabilities(&self) -> ModelCapabilities;

    async fn invoke(
        &self,
        request: ModelRequest,
    ) -> Result<ModelResponse, ModelError>;

    async fn stream(
        &self,
        request: ModelRequest,
    ) -> Result<ModelStream, ModelError>;
}
```

额外能力独立为服务或扩展 Trait：

```text
ModelCatalog
ModelRouter
PricingProvider
UsageCalculator
RetryPolicy
ModelCache
CredentialProvider
```

模型选择应当体现在不可变的请求或 `ModelRef` 中，而不是修改共享 Provider：

```rust
runner.run(
    &agent,
    input,
    RunConfig::default().model("claude-sonnet"),
);
```

---

## 6. `AgentResponder` 承担了过多 Runner 生命周期职责

当前 `AgentResponder` 不仅负责模型响应，还包含：

* signal 检查；
* LLM 调用前处理；
* 文本结果处理；
* iteration 后处理；
* Tool intent nudge；
* policy response。

这使 Provider 可以直接干预整个 Agent 循环，模型适配器和编排器边界不清晰。

建议拆分：

```text
Model
  只负责模型请求和响应

Runner
  负责 Agent 循环和状态机

Middleware
  负责前后拦截

RunController
  负责 cancel、steer、approve
```

---

## 7. 安全能力存在，但没有形成不可绕过的执行链

当前公共契约存在矛盾：

* `ToolExecutor` 文档认为实现者要负责审批、Egress 和沙箱；
* `SequentialDispatcher` 又在外层做审批和 Egress；
* `ToolToExecutorAdapter` 明确不读取 `Tool::requires_approval()` 和风险信息；
* 使用者可以直接实现或调用底层 executor，绕开策略链。

作为 SDK，安全能力必须是结构性的，而不是约定性的。

推荐唯一执行路径：

```text
ToolRegistry
    ↓
参数反序列化与 Schema 验证
    ↓
BeforeTool Middleware
    ↓
权限与策略决策
    ↓
审批或中断
    ↓
参数重写/脱敏
    ↓
沙箱或连接器执行
    ↓
结果清洗
    ↓
AfterTool Middleware
    ↓
审计与 ToolResult Event
```

公共 API 只暴露受控的 `ToolRuntime`。未检查的 Raw Executor 应当是私有接口，或者明确命名为：

```rust
UnsafeToolExecutor
TrustedToolBackend
```

工具输入输出 Guardrail 应分别位于执行前后，而不是只检查模型输出。成熟 SDK 也普遍把 Tool Input/Output Guardrail 作为工具调用外围的标准执行阶段。([OpenAI GitHub][2])

---

# 三、推荐的目标架构

```text
┌─────────────────────────────────────────┐
│                dasclaw                  │
│      面向用户的稳定 Facade 与 Prelude     │
└──────────────────┬──────────────────────┘
                   │
        ┌──────────▼──────────┐
        │   dasclaw-runtime   │
        │ Runner / RunState   │
        │ Loop / Events       │
        │ Middleware Pipeline │
        └──────────┬──────────┘
                   │
        ┌──────────▼──────────┐
        │     dasclaw-core    │
        │ Agent / Model Trait │
        │ Tool Trait / Input  │
        │ Output / Error / ID │
        └─────────────────────┘

可选集成：

dasclaw-provider-openai
dasclaw-provider-anthropic
dasclaw-provider-ollama
dasclaw-session-postgres
dasclaw-mcp
dasclaw-guardrails
dasclaw-otel
dasclaw-sandbox
dasclaw-tool-fs
dasclaw-tool-shell
dasclaw-tool-office
```

## `dasclaw-core` 中应该有什么

只放稳定、轻量的协议和 Trait：

* `Agent`
* `Model`
* `ModelRequest/Response`
* `Tool`
* `ToolRegistry`
* `RunInput`
* `RunOutput`
* `RunEvent`
* `RunId/TurnId/ToolCallId`
* `Usage`
* `Error`
* `Context`
* 结构化输出接口

不应该包含：

* reqwest
* PostgreSQL
* Keychain
* App Server
* 项目文档管理
* 具体 Provider
* Bash 实现
* GUI/desktop 类型
* 具体企业数据库

## `dasclaw-runtime` 中应该有什么

* `Runner`
* Agent 循环；
* `RunState`
* `RunHandle`
* Middleware 执行；
* Tool 调度；
* Session checkpoint 协议；
* cancellation；
* approval interruption；
* event stream；
* usage/budget enforcement。

Runtime 应只依赖 `core`，以及少量通用异步依赖。

---

# 四、建议的核心公共 API

## 1. Agent 是不可变定义

```rust
pub struct Agent<C, O = String> {
    id: AgentId,
    name: String,
    instructions: Instructions<C>,
    model: Arc<dyn Model>,
    tools: ToolRegistry<C>,
    output: OutputSchema<O>,
    guardrails: Vec<Arc<dyn Guardrail<C>>>,
    handoffs: Vec<Handoff<C>>,
}
```

不要让它持有：

* cancellation token；
* signal receiver；
* approval inbox；
* 当前 messages；
* 当前 tool calls；
* 当前 iteration。

## 2. Runner 持有基础设施

```rust
pub struct Runner<C> {
    session_store: Arc<dyn SessionStore>,
    tracer: Arc<dyn Tracer>,
    policy: Arc<dyn PolicyEngine<C>>,
    middleware: MiddlewareStack<C>,
    limits: RuntimeLimits,
}
```

## 3. 每次执行产生独立 RunHandle

```rust
pub struct RunHandle<O> {
    run_id: RunId,
    events: EventStream,
    control: RunControl,
    result: RunResultFuture<O>,
}
```

至少支持：

```rust
handle.cancel().await?;
handle.steer("优先参考最新制度").await?;
handle.approve(approval_id).await?;
handle.reject(approval_id, "权限不足").await?;
```

## 4. 每个事件必须可关联

当前事件缺少足够的调用关联标识。建议：

```rust
pub struct RunEvent {
    pub run_id: RunId,
    pub turn_id: TurnId,
    pub sequence: u64,
    pub timestamp: SystemTime,
    pub span_id: Option<SpanId>,
    pub kind: RunEventKind,
}
```

工具事件必须包含：

```rust
tool_call_id
tool_name
attempt
parent_span_id
```

否则并行调用两个同名工具时，前端和审计无法可靠匹配开始与结果。

Provider 特有事件不要直接加入稳定枚举，可以使用：

```rust
RunEventKind::Custom {
    namespace: String,
    payload: Value,
}
```

---

# 五、现有 crate 应如何调整

| 当前 crate                              | 建议                                                       |
| ------------------------------------- | -------------------------------------------------------- |
| `dasclaw_core`                        | 拆出真正轻量 Core；移除 `project_docs`、governance、外部 desktop 路径依赖 |
| `dasclaw_runtime`                     | 保留 Agent loop，重构为 Runner；移出具体 Provider、Keychain、数据库      |
| `dasclaw_tool`                        | 与 `runtime::Tool` 合并成唯一的公共 Tool API                      |
| `dasclaw_llm_provider`                | 拆成 Model Trait 与多个 Provider 集成 crate                     |
| `dasclaw_session`                     | 通过 Runner 接入，不再直接操作 `ReasoningContext`                   |
| `dasclaw_safety` / governance / hooks | 收敛成 Guardrail、Policy 和 Middleware 三层                     |
| `dasclaw_mcp`                         | 作为可选集成包，适配统一 ToolRegistry                                |
| `dasclaw_app_server*`                 | 作为参考 Host，不进入 SDK 默认依赖                                   |
| CLI                                   | 作为 SDK 示例和调试工具                                           |
| fs/git/shell/net/image 工具             | 变成独立可选 Tool Pack                                         |
| observability                         | 变成 Tracer Trait + OTel 插件                                |

53 个 crate 本身不是问题，问题是它们现在的公共边界不清晰。可以保留大量内部 crate，但建议使用者默认只接触：

```text
dasclaw
dasclaw-core
dasclaw-runtime
dasclaw-session
dasclaw-mcp
各 provider/tool 集成包
```

---

# 六、Session 需要从“消息快照”升级为“可恢复运行”

当前 Snapshot 主要保存：

* messages
* model
* prompt history
* workspace
* compaction

真正的 Agent SDK 还应保存：

* `agent_id` 和 Agent 定义版本；
* instructions hash；
* Tool manifest hash；
* Model config hash；
* Policy version；
* 当前 turn 和 sequence；
* pending tool calls；
* pending approval；
* 已执行工具的幂等键；
* usage 和预算；
* interruption reason；
* checkpoint version。

否则恢复会话时，可能使用已经改变的工具或 Agent 配置继续运行，出现不可复现行为。

建议区分：

```text
Conversation
  面向用户的历史会话

Run
  一次 Agent 执行

Checkpoint
  Run 的可恢复状态

Trace
  不可变的执行记录
```

---

# 七、公共 API 的兼容性需要重新治理

目前大量公共结构体字段直接 `pub`，枚举也没有 `#[non_exhaustive]`。这会导致以后增加字段或事件类型时形成破坏性升级。

建议：

* 公共结构使用私有字段和 Builder；
* 用户输入结构可使用 `Default`；
* 可扩展枚举和错误增加 `#[non_exhaustive]`；
* Rust 内部 Error 与跨进程 `ErrorReport` 分开；
* Provider 原始错误放入 `source()`，不要只转字符串；
* 稳定类型与实验类型分包；
* 为实验 API 增加 `experimental` feature。

Rust 官方建议使用 `#[non_exhaustive]` 为公共结构和枚举保留未来添加字段或变体的空间。([Rust 文档][3])

---

# 八、发布工程当前也不符合 SDK 状态

从归档内容看：

* 没有根 `Cargo.toml` Workspace；
* 没有完整根 `Cargo.lock`；
* 53 个 crate 中大多数 `publish = false`；
* 版本存在 `0.0.0-w1`、`w5`、`p0`、`0.1.0` 等多套；
* 同时使用 edition 2021 和 2024；
* `dasclaw_core` 有指向归档外部目录的 path dependency。

发布前至少需要：

```text
统一 Workspace
统一 edition 和 MSRV
统一版本策略
所有公开依赖声明 version
消除指向仓库外部的 path dependency
cargo package 全量检查
cargo semver-checks
最小 feature 和全 feature 构建
docs.rs 构建
公共 API diff
示例编译测试
```

Cargo 官方说明，发布到 crates.io 的依赖不能只指向仓库外部路径；内部 crate 通常应同时声明 `path` 和 `version`，本地开发使用路径，发布时使用 registry 版本。([Rust 文档][4])

---

# 九、推荐的迁移顺序

## P0：先建立新 SDK 边界

1. 新建 Facade crate：`dasclaw`。
2. 定义新的 `Agent`、`Runner`、`RunHandle`、`Model`、`Tool`。
3. 不立即删除旧实现，为旧运行时编写兼容 Adapter。
4. 冻结旧公共 API，不再继续向旧接口增加功能。
5. 建立根 Workspace、版本规则和依赖方向检查。

## P1：解决结构性问题

1. 将 Agent 定义与 Run 状态彻底分离。
2. 将 cancel、steer、approve 改成 run-local。
3. 合并 Tool、ToolDefinition、ToolExecutor 和 ToolDispatcher。
4. 建立唯一的 ToolRegistry 和 ToolRuntime。
5. 将具体 Provider、Keychain、数据库移出 Runtime。
6. 用泛型 `RunContext<C>` 替代 `JobContextCore`。

## P2：形成完整 SDK 能力

1. 结构化输出；
2. 多模态输入；
3. Agent handoff / sub-agent；
4. durable run 和 interruption；
5. Tool 输入输出 Guardrail；
6. tracing 和 usage；
7. 并行工具调用；
8. budget、timeout 和 retry；
9. Provider capability negotiation。

## P3：形成 SDK 产品

1. 完整 API 文档；
2. 最小示例；
3. 自定义 Model 示例；
4. 自定义 Tool 示例；
5. Streaming、Approval、Session、MCP 示例；
6. 独立测试工具包；
7. 兼容性和弃用策略；
8. Reference App Server 和 CLI。

---

# 十、重构后的 SDK 验收标准

可以用下面这些标准判断是否真正完成 SDK 化：

* `cargo add dasclaw` 后可以直接创建 Agent；
* Hello World 不超过约 20 行；
* 自定义强类型 Tool 不超过约 30 行；
* Tool 作者只依赖轻量 Core，不必依赖数据库和 Keychain；
* 同一个 Agent 可以安全并发执行多个 Run；
* cancel、steer、approve 都严格绑定 `run_id`；
* Schema 和 Tool 实现无法分离注册；
* 所有工具执行都经过唯一 Middleware 链；
* Core 不依赖 HTTP 客户端、数据库、OS Keychain 和具体 Provider；
* 每个事件都有 `run_id`、`turn_id`、`sequence`；
* Session 可以恢复 pending approval 和 pending tool；
* `cargo package`、文档测试和 SemVer 检查全部通过；
* 默认安全配置不能无意绕过审批、Guardrail 和沙箱。

## 最终建议

最适合你的定位是：

> **DasClaw：一个 Rust-first、可嵌入、Provider 无关、工具类型安全、支持持久运行和企业策略扩展的 Agent SDK。**

不要把它直接定位成“政企办公 Agent SDK”，否则 Core 很容易再次混入部门、审批、公文、租户等产品概念。更合理的分层是：

```text
DasClaw Agent SDK
        +
DasClaw Enterprise Extension
        +
Office Tool Pack / OA Connectors
        =
政企办公助手平台
```

当前代码大约 **70% 的底层实现可以保留**，但现有公共接口不建议直接作为 1.0 SDK 发布。先建立一套新的 Facade API，通过 Adapter 驱动旧 Runtime，再逐步替换内部实现，是风险最低的调整路线。

[1]: https://openai.github.io/openai-agents-python/agents/ "Agents - OpenAI Agents SDK"
[2]: https://openai.github.io/openai-agents-python/guardrails/ "Guardrails - OpenAI Agents SDK"
[3]: https://doc.rust-lang.org/reference/attributes/type_system.html "Type system - The Rust Reference"
[4]: https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html "Specifying Dependencies - The Cargo Book"
