app-server是否要剥离出来? 因为我看app-server好像比较靠近业务层?

是。**应从 Agent SDK 的发布面和默认依赖中剥离出来，但保留为上层 Host/参考应用。**

不建议现在拆成独立仓库。先放在同一个 Workspace 的 `apps/` 下，更利于它作为 SDK 的集成测试和示例应用。

## 当前判断

现在依赖方向基本正确：

```text
dasclaw_app_server → dasclaw_runtime → dasclaw_core
```

`runtime` 没有反向依赖 `app-server`，这是好事。

但 `app-server` 内部职责过多：

* JSON-RPC/stdio 协议处理；
* Thread、Turn 生命周期；
* Runtime 启停、取消、steer、审批；
* 模型 Provider 配置；
* Job、Skills、MCP；
* 文件系统、命令执行；
* Git、搜索、配置；
* 通知和健康检查；
* 具体服务实例化。

`dasclaw_app_server/src/lib.rs` 已约 1.95 万行，`app_server_protocol/src/lib.rs` 也约 7900 行，已经明显成为一个“Host + 业务控制面 + Runtime Adapter”的综合体。

## 最关键的问题

### 1. App Server 正在代替 Runtime 管理 Run

当前 `DasclawAgentRuntimeBridge` 自己维护：

```rust
in_flight: HashMap<TurnId, CancellationToken>
active_agents: HashMap<TurnId, Arc<Agent>>
pending_approvals: HashMap<RequestId, Arc<Agent>>
pending_client_requests: ...
```

这说明 SDK Runtime 还没有提供完整的：

```text
Runner
RunHandle
RunEventStream
RunControl
PendingInterruption
```

所以 App Server 被迫直接管理 `Agent`、取消令牌和审批。

SDK 化以后，这些状态应由 Runtime 管理：

```rust
let handle = runner.start(&agent, input, context).await?;

handle.cancel().await?;
handle.steer(input).await?;
handle.resolve(interruption_id, response).await?;
```

App Server 只保存 `RunHandle` 或 `RunId`，不应持有 `Arc<Agent>`。

---

### 2. App Server 服务接口直接依赖 RPC DTO

例如 `AppServerServices` 中：

```rust
trait FsService {
    fn read_file(
        &self,
        params: FsReadFileParams,
    ) -> Result<FsReadFileResponse, AppServerError>;
}
```

这里：

* `FsReadFileParams` 是协议层类型；
* `FsReadFileResponse` 是协议层类型；
* `AppServerError` 是服务器错误。

这意味着服务接口无法脱离 App Server 复用。

更合理的是：

```rust
trait FileService {
    async fn read(
        &self,
        request: ReadFileRequest,
    ) -> Result<FileContent, FileServiceError>;
}
```

然后由 App Server Adapter 完成：

```text
RPC Params
  → Domain Request
  → Service
  → Domain Result
  → RPC Response
```

不过文件、Git、模糊搜索等接口本身偏 GUI 产品功能，不一定需要进入通用 SDK。

---

### 3. App Server 在库代码中完成具体业务组装

`AppServerServices::real_with_root()` 直接创建：

* `ContextManager`
* JobService
* SkillsService
* MCP Service
* FsService
* CommandService
* ConfigService
* RepoService
* SearchService

这属于 **Composition Root**，应放在二进制入口或具体产品 Host 中，而不是通用 Server Library 中。

理想形式：

```rust
let runner = Runner::builder()
    .model_registry(models)
    .session_store(session_store)
    .policy_engine(policy)
    .build()?;

let services = DesktopServices::builder()
    .filesystem(fs)
    .command(command)
    .search(search)
    .build();

AppServer::builder()
    .runner(runner)
    .services(services)
    .transport(StdioTransport::new())
    .build()
    .serve()
    .await?;
```

## 推荐边界

### Agent SDK 负责

```text
Agent 定义
Runner / RunHandle
Run 状态机
模型调用
Tool Registry 和 Tool Runtime
Session / Checkpoint
Approval / Interruption
Guardrail / Policy 接口
Tracing / Usage
RunEvent
```

### App Server 负责

```text
JSON-RPC、stdio、HTTP、WebSocket
协议版本和能力协商
请求反序列化与参数校验
认证信息转换为 ExecutionContext
RPC DTO 与 SDK 类型转换
事件订阅、推送、背压
HTTP/RPC 错误映射
健康检查
```

### 具体产品或业务 Host 负责

```text
Jobs
Skills 管理
配置管理
文件系统浏览
PTY/命令控制
Git Diff
模糊文件搜索
MCP Server 管理
桌面客户端兼容
Office、OA、邮件、日历连接器
```

## 推荐项目结构

不必继续无限拆 crate，初期可收敛为：

```text
crates/
├── dasclaw
├── dasclaw-core
├── dasclaw-runtime
├── dasclaw-session
├── dasclaw-mcp
├── dasclaw-provider-*
└── dasclaw-tool-*

hosts/
├── dasclaw-app-server-protocol
├── dasclaw-app-server
└── dasclaw-desktop-host

apps/
└── dasclaw-app-server-bin
```

依赖方向：

```text
dasclaw-core
      ↑
dasclaw-runtime
      ↑
dasclaw 公开 Facade
      ↑
app-server runtime adapter
      ↑
desktop-host / server binary
```

`app-server` 必须始终是叶子节点，SDK 不能依赖它。

## 对现有模块的具体处理

| 当前内容                          | 建议归属                                |
| ----------------------------- | ----------------------------------- |
| `RuntimeBridge`               | 用 SDK 的 `Runner/RunHandle` 取代       |
| `DasclawAgentRuntimeBridge`   | 暂时保留为兼容 Adapter，最终简化                |
| `active_agents/in_flight`     | 移入 Runtime 的 Run Registry           |
| `pending_approvals`           | 移入 Runtime 的 interruption 管理        |
| `RuntimeTurnUpdateSink`       | 替换为通用 `RunEventStream`              |
| `thread_lifecycle.rs`         | 拆分处理                                |
| Thread 的消息、Checkpoint         | `dasclaw-session`                   |
| Thread 名称、归档、订阅、UI 元数据        | App Server/产品层                      |
| `fs_service.rs`               | Desktop Host                        |
| `command_service.rs`          | Desktop Host；不进入 SDK 默认能力           |
| `repo_service.rs`             | Desktop Host 或独立 Tool Pack          |
| `search_service.rs`           | 产品服务或独立 Knowledge/Search 集成         |
| `skills_service.rs`           | Host 插件管理层                          |
| `config_service.rs`           | Binary/Host 配置层                     |
| `job_service.rs`              | 持久任务框架成熟后再判断是否下沉                    |
| `mcp_service.rs`              | MCP 执行能力在 SDK；MCP Server 管理界面在 Host |
| JSON-RPC 类型                   | App Server Protocol                 |
| `CodexThread/CodexTurn` 等兼容类型 | 单独兼容模块，不进入 SDK API                  |

## `app-server-protocol` 也不应成为 SDK 核心协议

当前协议明显面向本地 GUI 控制面，包含：

* Thread/Turn；
* Command Execution；
* File Change；
* Git；
* Fuzzy Search；
* Skills；
* 配置；
* Codex 兼容结构。

它不是通用 Agent SDK API。

可以继续保留，但建议明确命名为：

```text
dasclaw-desktop-protocol
```

或者拆成：

```text
dasclaw-jsonrpc            通用 JSON-RPC 封装
dasclaw-agent-host-protocol 运行、取消、审批、事件
dasclaw-desktop-protocol     FS、Command、Git、Search、Skills
```

若协议不需要供第三方客户端使用，可以暂时保持一个 crate，只要不从 `dasclaw` Facade 导出即可。

## 是否需要独立仓库

目前不需要。

建议：

* 仍处于同一 Monorepo；
* `app-server` 保持 `publish = false`；
* 不加入 SDK 默认 feature；
* SDK 文档不要求用户理解 App Server；
* App Server 只能通过 SDK 的公开 API 工作；
* CI 禁止 App Server 使用 Runtime 私有模块。

这样 App Server 会成为非常好的“SDK 自测应用”：

> 如果 App Server 无法只使用公开 SDK API 实现，说明 SDK 公共能力仍不完整。

等到以下情况出现，再考虑独立仓库：

* App Server 与 SDK 有独立团队；
* 发布节奏明显不同；
* App Server 变成单独产品；
* 协议需要独立版本和兼容周期；
* SDK 用户完全不需要拉取 Host 源码。

## 推荐迁移顺序

1. 先明确 `app-server` 是 Host，不属于 SDK 默认包。
2. 在 Runtime 中完成 `Runner → RunHandle → RunEventStream`。
3. 用 `RunHandle` 替代 `RuntimeBridge` 对 `Agent`、CancellationToken 和审批状态的直接管理。
4. 将 `AppServerServices::real()` 移到 `main.rs` 或 `desktop-host`。
5. 将 FS、Command、Repo、Search、Config 等具体实现移出协议路由层。
6. 将 Session Checkpoint 下沉到 SDK，保留 UI Thread 元数据在 App Server。
7. 最后拆解 1.95 万行的 `lib.rs`，而不是一开始就机械增加大量 crate。

最终定位应当是：

```text
DasClaw Agent SDK
    提供 Agent 执行能力

DasClaw App Server
    把 SDK 暴露为 RPC 服务

DasClaw Desktop/Enterprise Host
    注入具体业务、连接器和部署能力
```

因此，**App Server 应剥离，但应作为 SDK 上层的标准 Host 和参考实现保留下来。**
