这是我还未完成的项目, 你帮助我分析一下, 想做一个政企级的办公助手的agent, 当前的能力是否够用, 这个agent框架目前缺哪些能力, 架构设计是否合理? 有哪些不足

# 总体判断

**目前还不足以直接作为政企级办公助手上线。**

更准确地说：

* 作为“安全执行型 Agent 内核”：约 **6/10**，方向正确，已经有相当多可复用能力。
* 作为“政企级办公助手平台”：约 **3/10**，仍处于内部 Alpha/技术预览阶段。
* 可用于：单用户、本地环境、低敏数据、受控内部试点。
* 暂不适合：多部门、多租户、敏感政务数据，以及自动执行发文、审批、邮件发送、业务系统写入等高风险动作。

最核心的问题不是“工具还不够多”，而是尚未形成完整的：

> **身份 → 权限 → 数据分级 → Agent 决策 → 工具执行 → 人工审批 → 审计追责**

闭环。

政企级判断不能只看 Agent 是否能调用工具。个人信息保护强调目的限定、最小必要和处理责任；数据安全要求分类分级和全流程安全管理；2026 年起施行的修改后《网络安全法》进一步强调人工智能风险监测评估和安全监管。NIST AI RMF 与 OWASP Agentic Security 指南也都把风险治理视为系统全生命周期能力，而非一个可选插件。([中国网络安全审查办

][1])

> 范围说明：本次属于静态源码审查。压缩包根目录缺少 `Cargo.toml`、`Cargo.lock`、README、CI 和部署文件，且 `dasclaw_core` 存在指向未打包上层目录的路径依赖，所以无法完成整仓构建验证。如果这些文件存在于未打包的父仓库，工程化评分会有所提高，但下面的安全和平台能力缺口仍然成立。

# 当前做得比较好的部分

| 能力领域         |   评价 | 判断                                                     |
| ------------ | ---: | ------------------------------------------------------ |
| Agent 循环与运行时 |   较强 | 已有流式输出、取消、工具调用、审批接口、循环控制、上下文压缩等基础                      |
| 模型接入层        |   较强 | 多 Provider、重试、故障转移、熔断、缓存、模型注册和路由体系较完整                  |
| 工具抽象         |   较强 | 文件、Git、Shell、网络、图像、LSP、MCP、WASM、子 Agent 等覆盖广           |
| 沙箱基础         |   中等 | Linux 沙箱、seccomp、bwrap/Landlock、WASM 能力边界等方向正确         |
| 安全扩展接口       |   中等 | Egress Gate、Approval、SecretProvider、Sanitizer 等接口设计有价值 |
| 知识检索原型       | 中等偏弱 | 已有 PostgreSQL、pgvector、FTS、Embedding、混合检索和多 scope      |
| 测试意识         |   较好 | 有较多单元测试和 CLI 端到端安全测试                                   |
| 企业平台能力       |    弱 | 身份、租户、权限、审计、持久工作流、办公连接器尚未闭环                            |

其中最值得保留的是：

* `dasclaw_runtime` 的 Agent/Responder/ToolExecutor 抽象；
* `dasclaw_llm_provider` 的多模型接入层；
* 工具能力与 WASM/MCP 扩展机制；
* Linux 沙箱、网络出口和密钥注入相关基础；
* 协议与客户端、服务端分离的方向。

不建议重写这些底层，建议围绕它们补齐企业控制面。

# 必须先解决的生产阻断问题

## 1. 工具参数安全闸门存在实际执行缺陷

在 `dasclaw_runtime/src/tool_dispatch.rs:124-175`：

1. `ToolCallStart` 事件先携带原始参数发出；
2. 之后才执行 Egress Gate；
3. Gate 修改的是序列化后的 `args_buf`；
4. 最终执行器调用的仍是原始 `call`。

也就是说：

* 工具参数里的密钥、个人信息可能在安全检查之前进入 UI、日志或事件总线；
* 即便安全闸门返回了脱敏参数，脱敏结果也没有真正传给工具执行器。

这是明确的 **P0 问题**。

正确顺序应当是：

```text
模型生成 ToolCall
  → 参数结构校验
  → 权限与数据策略检查
  → 参数脱敏/重写
  → 生成 SafeToolCall
  → 只发送脱敏后的事件
  → 使用 SafeToolCall 执行
```

不能只改字符串副本，必须将清洗后的 JSON 重新解析为实际执行参数。

## 2. PTY 流式命令执行明确未沙箱化

`dasclaw_app_server/src/command_service.rs` 中：

* `336-339`：流式 PTY 不支持 `sandboxPolicy` 和 `permissionProfile`；
* `387-399`：直接调用 PTY 后端启动进程；
* `471-479`：健康信息明确标记 `streaming_sandbox=none/unsandboxed`。

同时：

* `dasclaw_app_server/src/lib.rs:5154-5163` 报告 Runtime Bridge 的 `sandbox: false`；
* `5316-5320` 对非空 sandbox context 直接报“不支持执行”。

这意味着一旦办公助手允许模型触发终端命令，可能绕开你已有的大部分沙箱设计。

**生产配置中应立即彻底禁用 PTY 流式执行**，直到它能够运行在与 buffered execution 相同或更强的隔离边界内。

## 3. 默认配置是 Fail-open，而不是 Fail-closed

当前默认行为包括：

* `dasclaw_runtime/src/agent.rs:882-899`：未配置审批时使用 `NoApprovalPolicy`，自动放行；
* `dasclaw_core/src/hooks.rs:368-378`：默认是 Noop Egress、Noop Sandbox、内存密钥、自动审批；
* `dasclaw_runtime/src/agent.rs:795-799`：未配置输出清洗器时，工具输出原样进入下一轮模型上下文。

这些默认值用于单元测试可以理解，但作为公开 Builder 默认值非常危险。调用方只要漏配一个组件，安全边界就消失了。

建议增加明确的运行模式：

```text
TestProfile       允许 Noop
DeveloperProfile  有风险提示，限制高危工具
EnterpriseProfile 所有安全组件必填，缺一项启动失败
```

当存在 ToolExecutor 时，企业模式必须强制要求：

* 身份上下文；
* 策略引擎；
* 审批策略；
* 沙箱执行器；
* 出口检查；
* 输出清洗；
* 审计记录器。

## 4. 身份、租户和授权几乎没有真正落地

`dasclaw_identity/src/lib.rs:1-17` 明确写的是 skeleton，只有一个占位 `sign()` trait。

`ironclaw_auth/src/jwt.rs:6-19` 中 JWT claims 只有：

```text
sub / exp / iat / token_type
```

缺少政企系统常用的：

* `tenant_id`；
* 组织与部门；
* 角色、scope、数据权限；
* `iss`、`aud`、`jti`；
* 会话与设备信息；
* 授权链与委托身份；
* Token 撤销、密钥轮换和 JWKS；
* MFA；
* OIDC、SAML、LDAP/AD；
* 机器身份和服务间 mTLS。

尤其要区分：

* 登录用户；
* 当前代理用户；
* Agent 服务身份；
* 工具执行身份；
* 目标系统中的委托身份。

MCP 虽然已有不少 OAuth 代码，但 App Server 的 OAuth 登录仍明确标记为未接通。正式 MCP 授权还必须做资源与 audience 绑定，禁止把收到的 Token 原样透传给下游服务。([Model Context Protocol][2])

## 5. DLP 不是完整的端到端数据防护

`dasclaw_core/src/agentic_loop.rs:555-573` 只检查最近一条用户消息，而不是实际发送给模型的完整请求。

完整请求还可能包含：

* System Prompt；
* 历史消息；
* 工具输出；
* 检索结果；
* 附件提取文本；
* MCP 返回内容；
* 隐式注入的密钥或业务上下文。

此外，`dasclaw_app_server/src/lib.rs:2895-2898` 明确表示 DLP/Policy Service 尚未迁移。

政企场景需要在以下每个边界分别做检查：

```text
用户输入
检索文档
模型请求
模型响应
工具参数
工具输出
外部网络请求
持久化内容
审计内容
最终用户展示
```

而且不能只依赖“匹配手机号/密钥格式”的正则表达式，还需要数据来源、数据分类、使用目的和允许流向等上下文。

# 企业知识库仍停留在原型阶段

你的项目并不是完全没有 RAG。`dasclaw_workspace_cap` 已经实现了：

* PostgreSQL/pgvector；
* 全文检索与向量检索；
* RRF/加权融合；
* OpenAI/Ollama Embedding；
* 多 scope 查询。

但距离政企知识平台还有明显差距。

## 当前问题

1. PostgreSQL 能力默认关闭：`dasclaw_workspace_cap/Cargo.toml:58-67`。

2. 全文检索硬编码为英文：

   `repository.rs:432-445` 和 `551-565` 使用：

   ```sql
   plainto_tsquery('english', ...)
   ```

   对中文公文、制度文件和混合语言检索不合适。

3. 权限过滤只有 `user_id`、scope 和可选 `agent_id`，没有：

   * 部门与岗位；
   * 文档级 ACL；
   * 密级；
   * 项目成员；
   * 有效期；
   * 原系统权限版本；
   * 动态 ABAC 条件。

4. `chunker.rs:69-114` 以空白词为单位切块，不适合中文、表格、公文章节、附件标题和 Excel 工作表。

5. `SearchResult` 只有文档路径、chunk、内容和分数，缺少：

   * 页码、段落、表格和工作表位置；
   * 文档版本与内容哈希；
   * 来源系统；
   * ACL 快照；
   * 引用证据；
   * 生效、废止时间。

6. 静态搜索未发现该检索模块真正进入 Agent/App Server 主执行链。

7. 在当前压缩包中未发现完整的 DOCX、XLSX、PPTX、扫描 PDF、邮件和公文版式解析链路。

政企 RAG 的关键原则是：

> **先按用户真实权限过滤，再向量检索；不能先检索出来，再让模型判断是否有权查看。**

最好在数据库查询层直接施加 ACL 和数据分级条件，并保留每一次检索的权限快照和引用来源。

# 持久任务和办公流程能力不足

`dasclaw_routines/src/lib.rs:6-9` 明确指出以下部分仍未迁移：

* routine engine；
* scheduler；
* cost guard；
* heartbeat；
* job monitor；
* self-repair。

同时：

* Rate Limit 是进程内状态，重启后清空；
* Context Manager 主要使用内存 `HashMap`；
* Session 虽然定义了持久化接口，但压缩包中提供的主要实现仍偏内存型。

办公 Agent 不能只有“LLM 循环”。审批、发文、会议任务、通知、跨系统同步往往持续数小时甚至数天，需要：

* 持久状态机；
* 幂等键；
* Worker 租约与分布式锁；
* 重试策略；
* 超时与补偿；
* 人工任务状态；
* 死信队列；
* 重启恢复；
* 操作撤销或反向操作；
* “已生成、待审核、已批准、执行中、已完成、已驳回”的明确状态。

否则服务重启、网络超时或模型重复调用，都可能导致重复发邮件、重复创建工单或重复提交审批。

# 办公产品能力与现有工具方向不完全匹配

当前工具大量集中于：

* 文件和代码编辑；
* Git；
* Shell；
* LSP；
* Apply Patch；
* 网络抓取；
* 开发型子 Agent。

因此它更接近“代码/研发助手底座”。

政企办公助手还需要一层真正的办公领域能力：

* Word、Excel、PPT、PDF 的结构化读取与编辑；
* 公文模板、红头、版记、文号和格式校验；
* 邮件、日历、会议、联系人；
* OA、档案、ERP、CRM、工单和知识门户；
* 企业微信、钉钉、飞书、Teams 等渠道；
* 会议纪要与任务跟踪；
* 合同、制度和公文差异比对；
* 电子签章和审批流程；
* 每个连接器的用户委托授权与源系统权限继承。

这类连接器不能只是“封装一个 HTTP API”。每个写操作都要具备：

* 幂等性；
* 明确的操作预览；
* 可配置审批；
* 目标资源权限检查；
* 操作结果凭证；
* 可审计的请求和响应摘要。

# 审计与可观测性不足

`dasclaw_observability/src/lib.rs:1-13` 当前只有：

* Noop；
* tracing log；
* 多后端 fan-out。

默认配置是 `none`，OpenTelemetry 和 Prometheus 仍属于未来扩展。

政企审计至少应记录：

```text
tenant_id
actor_id / delegated_user_id
request_id / trace_id
模型与版本
Prompt 模板版本
策略版本
数据分类
检索文档及引用
工具名及参数摘要
权限决策
审批人和审批结果
外部系统操作结果
Token 与成本
异常、重试和补偿
```

审计记录还应写入独立、追加式、限制修改的存储，而不是普通应用日志。

OpenTelemetry 已有面向生成式 AI 的语义约定，可表达模型调用、Token、Agent 和工具执行链；但完整 Prompt、工具参数和结果可能包含敏感信息，因此内容采集应默认关闭，只保留必要元数据，并由专门策略启用。([OpenTelemetry][3])

# 架构设计是否合理

## 宏观方向是合理的

你已经意识到需要拆分：

* Runtime；
* Model Provider；
* Tool；
* Safety；
* Sandbox；
* MCP；
* Protocol；
* App Server；
* Session；
* Workspace Knowledge。

这是正确方向，Rust 也适合实现高可信的 Agent 执行内核。

## 但当前存在三个架构性问题

### 1. 安全责任分散，缺少唯一权威

目前权限与安全相关逻辑分散在：

* safety；
* governance；
* hooks；
* approval policy；
* bash permissions；
* execpolicy；
* sandbox；
* egress gate；
* workspace capability。

容易出现“每个模块都有检查，但没人知道哪个检查才是最终权威”的问题。

建议采用：

* 一个统一的 **策略决策点 PDP**；
* 在模型出口、检索、工具、连接器、密钥和最终输出处设置 **策略执行点 PEP**；
* 所有决策返回统一的 `PolicyDecision` 和 `decision_id`；
* 每个执行点必须把 `decision_id` 写入审计。

### 2. 缺少贯穿全链路的企业执行上下文

建议定义一个不可缺省的上下文：

```rust
pub struct ExecutionContext {
    pub tenant_id: TenantId,
    pub actor_id: SubjectId,
    pub delegated_user_id: Option<SubjectId>,
    pub org_path: Vec<OrgId>,
    pub roles: Vec<RoleId>,
    pub scopes: Vec<Scope>,
    pub purpose: ProcessingPurpose,
    pub data_classification: DataClassification,
    pub request_id: RequestId,
    pub trace_id: TraceId,
    pub policy_version: PolicyVersion,
    pub approval_chain_id: Option<ApprovalChainId>,
}
```

模型调用、知识检索、工具执行、连接器请求、密钥获取和审计接口都必须显式接收该对象。不能从全局变量、环境变量或可选字段中猜测。

### 3. 拆 crate 较多，但产品边界尚未稳定

当前有 53 个顶层 crate，其中部分仍是 skeleton 或迁移占位。同时：

* 根 Workspace 不完整；
* `app_server/src/lib.rs` 单文件约 1.95 万行；
* 多处仍依赖未迁移的 desktop 模块；
* app-server 入口明确是 Phase 1 的 stdio sidecar，而非正式网络控制平面。

这表明项目正在“拆代码”，但还没有完全形成稳定的服务和信任边界。

建议最终收敛为六个逻辑平面：

```text
渠道/UI
   │
API Gateway + SSO + Tenant Context
   │
Agent Orchestrator ───── Policy/Approval Service
   │                      │
   ├── Model Gateway      │
   ├── Knowledge Service  │
   └── Isolated Tool Workers
               │
     Audit Ledger + OpenTelemetry + Evaluation
```

其中高危工具执行进程应与 API、模型编排进程分离，使用短时、最小权限的能力凭证。

# 建议的改造优先级

| 优先级 | 必须完成的事项                              | 验收标准                          |
| --- | ------------------------------------ | ----------------------------- |
| P0  | 修复 ToolCall 参数闸门；关闭未沙箱 PTY；企业模式默认拒绝  | 不存在未检查参数执行路径，不存在未隔离命令路径       |
| P0  | 补齐根 Workspace、Lockfile、CI、依赖审计和可复现构建 | 一个命令完成全量构建与测试                 |
| P0  | 强制身份、租户和执行上下文                        | 每次模型、检索、工具调用都能定位租户和操作人        |
| P0  | 统一策略引擎和审批链                           | 所有高风险操作都有策略决策 ID 和审批凭证        |
| P1  | ACL-aware 企业知识库和中文文档链路               | 无权文档无法进入候选集，答案带页码和版本引用        |
| P1  | 持久工作流、幂等、重试和恢复                       | 服务重启不丢任务，不重复执行外部动作            |
| P1  | 企业 SSO、委托授权、MCP OAuth                | Token audience、scope、撤销和轮换可验证 |
| P1  | 独立审计账本和 OTel                         | 一次请求可完整追踪到模型、检索、审批和工具         |
| P2  | 邮件、日历、会议、OA、公文和 Office 连接器           | 读写操作均继承源系统权限并支持审批             |
| P2  | 模型治理和评测平台                            | 支持模型白名单、数据驻留、预算、回归评测和红队测试     |
| P3  | 高可用、容灾、限流、配额和运维控制台                   | 满足正式生产部署与故障恢复目标               |

# 最终建议

不要继续优先增加更多工具。当前最需要做的是把已有安全组件从“可选接口”升级成“无法绕过的系统边界”。

比较合适的产品切入顺序是：

1. **企业知识问答，必须带引用且继承权限**；
2. **会议纪要、制度总结、公文草拟等只读或草稿型任务**；
3. **流程发起和系统写入，但必须人工确认**；
4. 最后才开放自动发送、自动审批、命令执行等高风险动作。

总体上，这个项目值得继续做，底层并不差；但目前最大的短板不是 Agent 推理能力，而是企业身份、数据权限、持久流程、审计和安全组合尚未成为强制性的基础设施。建议下一步直接形成一份按文件划分的 P0 改造清单，以及统一的 `ExecutionContext`、`PolicyDecision` 和安全 Tool Dispatcher 接口草案。

[1]: https://www.cac.gov.cn/2021-08/20/c_1631050028355286.htm "中华人民共和国个人信息保护法_中央网络安全和信息化委员会办公室"
[2]: https://modelcontextprotocol.io/specification/2025-06-18/basic/authorization "Authorization - Model Context Protocol"
[3]: https://opentelemetry.io/blog/2026/genai-observability/ "Inside the LLM Call: GenAI Observability with OpenTelemetry | OpenTelemetry"
