# 按需引入 IronClaw 模块的可行性分析

> 生成日期：2026-03-21
> 核心问题：能不能客户端只引入需要的 IronClaw 模块，管理端也只引入需要的模块？
> 附带问题：如果 Sidecar 了完整的 IronClaw，数据存在 IronClaw 本地，怎么上报到管理端？

---

## 一、你的直觉是对的

你提出了两个关键问题：

1. **打包膨胀**：为什么要把整个 IronClaw 打包进客户端？客户端只需要 chat、skills、extensions 等少数能力。
2. **数据孤岛**：如果每个客户端都 Sidecar 一个完整的 IronClaw，数据存在各自的本地 libSQL 里，管理端怎么看到这些数据？

这两个问题其实指向同一个架构矛盾：**IronClaw 被设计为一个独立运行的单体服务，不是一个可拆分的库**。

---

## 二、"按需引入模块"为什么做不到（现状）

### IronClaw 的模块依赖关系

```
你想要的（理想情况）：
  客户端: use ironclaw::agent;        ← 只要 Agent
  客户端: use ironclaw::llm;          ← 只要 LLM
  客户端: use ironclaw::skills;       ← 只要 Skills
  编译 → 小二进制（~20MB）✅

实际情况：
  use ironclaw::agent
    → 需要 ironclaw::llm（LLM 推理）
    → 需要 ironclaw::tools（工具执行）
      → 需要 ironclaw::sandbox（Docker 沙箱）
      → 需要 ironclaw::tools::wasm（WASM 工具运行时）
        → 需要 wasmtime（~15MB）
      → 需要 ironclaw::tools::mcp（MCP 工具）
    → 需要 ironclaw::safety（安全过滤）
    → 需要 ironclaw::context（上下文管理）
    → 需要 ironclaw::db（数据库）
      → 需要 libsql + tokio-postgres
    → 需要 ironclaw::hooks（生命周期钩子）
    → 需要 ironclaw::workspace（记忆/工作空间）
    → 需要 ironclaw::extensions（扩展管理）
    → 需要 ironclaw::skills（技能系统）
    → 需要 ironclaw::secrets（密钥管理）
    → 需要 ironclaw::estimation（成本估算）
    → 需要 ironclaw::transcription（语音转文字）
    → 需要 ironclaw::document_extraction（文档提取）
  编译 → 还是很大（~60MB）❌
```

### 根本原因：Agent 是一个"上帝对象"

从代码分析，`AgentDeps` 结构体需要 15 个依赖：

```rust
pub struct AgentDeps {
    pub store: Option<Arc<dyn Database>>,           // 数据库
    pub llm: Arc<dyn LlmProvider>,                  // LLM
    pub cheap_llm: Option<Arc<dyn LlmProvider>>,    // 廉价 LLM
    pub safety: Arc<SafetyLayer>,                   // 安全层
    pub tools: Arc<ToolRegistry>,                   // 工具注册表
    pub workspace: Option<Arc<Workspace>>,          // 工作空间
    pub extension_manager: Option<Arc<ExtensionManager>>,  // 扩展管理
    pub skill_registry: Option<Arc<...>>,           // 技能注册
    pub skill_catalog: Option<Arc<SkillCatalog>>,   // 技能目录
    pub hooks: Arc<HookRegistry>,                   // 钩子
    pub cost_guard: Arc<CostGuard>,                 // 成本控制
    pub sse_tx: Option<...>,                        // SSE 广播
    pub http_interceptor: Option<...>,              // HTTP 拦截
    pub transcription: Option<...>,                 // 语音转文字
    pub document_extraction: Option<...>,           // 文档提取
}
```

而 `ToolRegistry` 内部又注册了 Docker 沙箱工具、WASM 工具等，这些都是编译时链接的。

### 为什么不能 `use ironclaw::skills` 单独用？

```rust
// 你想这样做：
use ironclaw::skills::SkillRegistry;

let registry = SkillRegistry::new(&config.skills);
let skills = registry.list_skills();

// 但 SkillRegistry 的实现依赖：
// 1. 文件系统扫描（读取 skills/ 目录下的 SKILL.md）
// 2. YAML 解析（serde_yml）
// 3. 这部分确实可以独立使用 ✅

// 问题是 SkillCatalog（在线技能市场）依赖：
// 1. HTTP 客户端（reqwest）
// 2. 注册表配置
// 这部分也可以独立使用 ✅

// 真正的问题是：你不只是要"列出技能"
// 你要"用技能和 Agent 对话"
// 这就需要完整的 Agent → LLM → Tools → ... 链条
```

### 模块耦合度分析

| 模块 | 能否独立使用 | 依赖链 |
|------|------------|--------|
| `skills` (列出/安装) | ✅ 可以 | 只需要文件系统 + HTTP |
| `extensions` (列出/安装) | ✅ 可以 | 只需要文件系统 + WASM runtime |
| `settings` (读写设置) | ✅ 可以 | 只需要数据库 |
| `safety` (内容过滤) | ✅ 可以 | 独立 crate (`ironclaw_safety`) |
| `agent` (对话/推理) | ❌ 不行 | 需要 LLM + Tools + DB + Safety + ... |
| `llm` (调用大模型) | 🟡 勉强 | 需要 config + secrets |
| `tools` (执行工具) | ❌ 不行 | 需要 WASM + Docker + MCP + ... |
| `channels/web` (Gateway API) | ❌ 不行 | 需要几乎所有模块 |

**结论**：管理类操作（列出技能、安装扩展、读写设置）可以独立使用，但核心的 AI 对话能力无法拆分。

---

## 三、数据孤岛问题（更关键）

这是你提出的第二个问题，也是更致命的问题。

### 当前架构的数据流

```
现在（开发环境）：
  ┌──────────────┐     HTTP      ┌──────────────┐
  │ Desktop      │ ──────────→   │ IronClaw     │
  │ Client       │ ←──────────   │ Server       │
  └──────────────┘    (38080)    │              │
                                 │  ┌─────────┐ │
                                 │  │ libSQL  │ │  ← 数据在这里
                                 │  │ 数据库   │ │
                                 │  └─────────┘ │
                                 └──────────────┘
                                       ↑
  ┌──────────────┐     HTTP      ┌─────┘
  │ Admin        │ ──────────→   │（管理端也连这个 IronClaw）
  │ Backend      │               │
  └──────────────┘               │
                                 ↓
                           所有数据在同一个地方 ✅
```

### 如果每个客户端 Sidecar 一个 IronClaw

```
Sidecar 方案的数据流：

  用户 A 的电脑：
  ┌──────────────────────────────┐
  │ Desktop Client               │
  │  └── IronClaw Sidecar        │
  │       └── libSQL (本地数据库)  │  ← 用户 A 的数据在这里
  └──────────────────────────────┘

  用户 B 的电脑：
  ┌──────────────────────────────┐
  │ Desktop Client               │
  │  └── IronClaw Sidecar        │
  │       └── libSQL (本地数据库)  │  ← 用户 B 的数据在这里
  └──────────────────────────────┘

  管理端：
  ┌──────────────────────────────┐
  │ Admin Backend                │
  │  └── SQLite (管理端数据库)    │  ← 管理端的数据在这里
  └──────────────────────────────┘

  问题：
  ❌ 管理端看不到用户 A 和用户 B 的对话记录
  ❌ 管理端看不到用户的 DLP 违规事件
  ❌ 管理端看不到用户的审计日志
  ❌ 管理端无法统一管理所有用户的技能/扩展
  ❌ 管理端无法推送安全策略到各个客户端
  ❌ 每个客户端都是一个数据孤岛
```

### 这才是真正的架构问题

Sidecar 方案的本质是：**每个客户端都运行一个独立的 IronClaw 实例，有自己的数据库、自己的配置、自己的状态**。

这对于"个人使用"是没问题的（一个人用一个 IronClaw），但对于"企业管理"场景是灾难性的：

| 管理需求 | Sidecar 方案能否满足 |
|---------|-------------------|
| 查看所有用户的对话记录 | ❌ 数据分散在各个客户端 |
| 统一 DLP 安全策略 | ❌ 每个客户端独立配置 |
| 审计日志集中管理 | ❌ 日志分散在各个客户端 |
| 统一管理技能/扩展 | ❌ 每个客户端独立安装 |
| 用户行为分析/报表 | ❌ 没有集中数据源 |
| 远程禁用某个用户 | ❌ 无法控制本地 Sidecar |

---

## 四、正确的架构应该是什么？

### 架构选择取决于产品定位

```
问题：IronClaw Desktop 是什么产品？

选项 A：个人工具（像 VS Code）
  → 每个人独立使用，不需要集中管理
  → Sidecar 方案完全可行
  → 不需要管理端

选项 B：企业工具（像 Slack/Teams）
  → 需要集中管理、统一策略、审计合规
  → 必须有中心化的服务端
  → 客户端连接到企业部署的 IronClaw 服务
```

从你的项目结构来看（有 Admin Backend、DLP、审计日志、RBAC），这明显是**选项 B：企业工具**。

### 企业场景的正确架构

```
正确的架构：

                    ┌─────────────────────────┐
                    │   IronClaw Server        │
                    │   (企业统一部署)           │
                    │                         │
                    │  ┌───────────────────┐   │
                    │  │ PostgreSQL/libSQL  │   │  ← 所有数据集中存储
                    │  │ (集中数据库)        │   │
                    │  └───────────────────┘   │
                    │                         │
                    │  Gateway API (:38080)    │
                    └────────┬────────────────┘
                             │
              ┌──────────────┼──────────────┐
              │              │              │
              ▼              ▼              ▼
  ┌───────────────┐ ┌───────────────┐ ┌───────────────┐
  │ Desktop       │ │ Desktop       │ │ Admin         │
  │ Client A      │ │ Client B      │ │ Backend       │
  │ (纯前端)      │ │ (纯前端)      │ │ (管理控制台)   │
  └───────────────┘ └───────────────┘ └───────────────┘

  所有客户端连接同一个 IronClaw Server
  管理端也连接同一个 IronClaw Server
  数据集中存储，统一管理 ✅
```

### 这个架构下，客户端不需要 Sidecar

```
Desktop Client 的角色：
  ├── Tauri 应用（壳）
  ├── 前端 UI（React/Vue）
  ├── Tauri Commands（HTTP 代理）
  │   ├── chat → 转发到 IronClaw Server /api/chat
  │   ├── skills → 转发到 IronClaw Server /api/skills
  │   ├── extensions → 转发到 IronClaw Server /api/extensions
  │   └── settings → 转发到 IronClaw Server /api/settings
  └── 本地功能（不需要 IronClaw）
      ├── 主密码管理
      ├── 本地配置
      └── 离线缓存

客户端安装包大小：~15MB（只有 Tauri 壳 + 前端资源）
不需要打包 IronClaw 二进制 ✅
不需要 Sidecar ✅
```

### 但是...用户需要自己部署 IronClaw Server

这就回到了最初的问题：

```
企业部署模式：
  IT 管理员部署 IronClaw Server（Docker/K8s/VM）
  → 所有员工的客户端连接到这个 Server
  → 管理端也连接到这个 Server
  → 数据集中，统一管理

个人使用模式：
  用户自己在本地运行 IronClaw
  → 客户端连接到 localhost:38080
  → 不需要管理端
  → 这时候 Sidecar 才有意义（方便个人用户）
```

---

## 五、两种产品模式的架构对比

### 模式 1：企业版（推荐）

```
部署方式：
  IronClaw Server → 企业服务器/云端
  Desktop Client → 员工电脑（纯前端，无 Sidecar）
  Admin Backend → 管理员使用

数据流：
  Client A ──→ IronClaw Server ←── Admin Backend
  Client B ──→ IronClaw Server
  Client C ──→ IronClaw Server

  所有数据在 IronClaw Server 的数据库中
  Admin Backend 通过 IronClaw API 或直连数据库获取数据
```

| 维度 | 评价 |
|------|------|
| 数据集中 | ✅ 所有数据在一个地方 |
| 统一管理 | ✅ 管理端可以管理所有用户 |
| 安全合规 | ✅ DLP、审计、RBAC 都能工作 |
| 客户端大小 | ✅ ~15MB（纯前端） |
| 部署复杂度 | 🟡 需要部署 IronClaw Server |
| 离线使用 | ❌ 需要网络连接 |

### 模式 2：个人版 + Sidecar

```
部署方式：
  Desktop Client + IronClaw Sidecar → 用户电脑
  无 Admin Backend（个人不需要管理端）

数据流：
  Client ──→ 本地 IronClaw Sidecar ──→ 本地 libSQL
  所有数据在用户自己的电脑上
```

| 维度 | 评价 |
|------|------|
| 数据集中 | ❌ 数据分散在各个电脑 |
| 统一管理 | ❌ 无法集中管理 |
| 安全合规 | ❌ 无法统一 DLP 策略 |
| 客户端大小 | 🟡 ~85MB（包含 IronClaw） |
| 部署复杂度 | ✅ 双击即用 |
| 离线使用 | ✅ 完全离线可用 |

### 模式 3：混合模式（最灵活）

```
企业用户：
  Client → 连接企业 IronClaw Server
  Admin Backend → 管理所有用户

个人用户：
  Client + Sidecar → 本地运行
  无 Admin Backend

客户端启动逻辑：
  1. 检查是否配置了企业 Server 地址
     ├── 是 → 连接企业 Server（企业模式）
     └── 否 → 启动本地 Sidecar（个人模式）
```

---

## 六、回答你的两个问题

### 问题 1：能不能按需引入 IronClaw 模块？

**短答案**：管理类操作可以，AI 对话能力不行。

**详细解释**：

```
可以独立引入的模块（管理类）：
  ✅ ironclaw::skills::SkillRegistry     → 列出/安装技能
  ✅ ironclaw::skills::catalog            → 搜索技能市场
  ✅ ironclaw_safety                      → 内容安全过滤（已是独立 crate）
  ✅ ironclaw::settings                   → 读写设置
  ✅ ironclaw::config                     → 配置管理

不能独立引入的模块（核心能力）：
  ❌ ironclaw::agent     → 依赖 LLM + Tools + DB + Safety + ...
  ❌ ironclaw::llm       → 依赖 Config + Secrets + 多个 Provider
  ❌ ironclaw::tools     → 依赖 WASM + Docker + MCP
  ❌ ironclaw::channels  → 依赖 Agent + 几乎所有模块
```

如果你只需要"管理"能力（列出技能、安装扩展、读写设置），可以直接引入对应模块。但如果需要"AI 对话"能力，就必须引入整个 Agent 链条，这基本等于引入了 80% 的 IronClaw。

### 问题 2：Sidecar 的数据怎么上报到管理端？

**短答案**：在企业场景下，不应该用 Sidecar。

**如果非要用 Sidecar + 数据上报**，需要额外实现：

```
方案 A：客户端主动上报（复杂）
  Client Sidecar → 本地 libSQL（主存储）
                 → 定时同步到管理端 API（上报）

  需要实现：
  1. 数据同步服务（增量同步、冲突解决）
  2. 上报 API（管理端接收数据）
  3. 离线队列（网络断开时缓存）
  4. 数据去重和一致性保证
  预估工作量：4-6 周

方案 B：双写（更复杂）
  Client Sidecar → 本地 libSQL（本地缓存）
                 → 同时写入管理端数据库（主存储）

  需要实现：
  1. 修改 IronClaw 的 Database trait 支持双写
  2. 网络失败时的重试和补偿
  3. 数据一致性保证
  预估工作量：6-8 周

方案 C：不用 Sidecar，直接连企业 Server（推荐）
  Client → 企业 IronClaw Server → 集中数据库
  Admin → 同一个数据库

  需要实现：
  0（已经是这样工作的）
  预估工作量：0
```

---

## 七、最终建议

### 如果你的产品是企业工具（有管理端、DLP、审计）

```
推荐架构：
  ┌─────────────┐
  │ IronClaw    │ ← 企业部署（Docker/K8s）
  │ Server      │
  │ (集中式)    │
  └──────┬──────┘
         │
    ┌────┼────┐
    │    │    │
    ▼    ▼    ▼
  Client Client Admin
  (纯前端) (纯前端) (管理端)

不需要 Sidecar
不需要按需引入模块
客户端只是一个连接 Server 的 UI 壳
管理端通过 IronClaw API 或直连数据库获取数据
```

**客户端要做的**：
- 连接配置（Server 地址、认证 Token）
- UI 界面（对话、技能管理、设置等）
- Tauri Commands 作为 HTTP 代理转发请求

**管理端要做的**：
- 连接同一个 IronClaw Server 的数据库
- 或通过 IronClaw Gateway API 获取数据
- 额外的管理功能（DLP 配置、用户管理、审计日志等）

### 如果你想同时支持个人用户

```
混合架构：
  客户端启动时：
  ├── 检测到企业配置 → 连接企业 Server（企业模式）
  └── 未检测到 → 启动本地 Sidecar（个人模式）

  个人模式下：
  - 不需要管理端
  - 数据存在本地
  - 接受全量打包 IronClaw（~85MB）
```

### 关于"按需引入模块"的长期建议

如果未来 IronClaw 要支持"库模式"（被其他项目引入），需要做以下重构：

```
ironclaw/
├── crates/
│   ├── ironclaw-core/        ← 核心抽象（traits, types）
│   ├── ironclaw-agent/       ← Agent 引擎
│   ├── ironclaw-llm/         ← LLM 抽象层
│   ├── ironclaw-tools/       ← 工具系统
│   ├── ironclaw-skills/      ← 技能系统（可独立使用）
│   ├── ironclaw-extensions/  ← 扩展系统
│   ├── ironclaw-gateway/     ← Web Gateway
│   ├── ironclaw-safety/      ← 安全过滤（已完成）
│   └── ironclaw-auth/        ← 认证（已完成）
└── src/
    └── main.rs               ← 组合所有 crates 的全功能二进制
```

这样客户端可以：
```toml
[dependencies]
ironclaw-skills = { path = "..." }      # 只要技能管理
ironclaw-extensions = { path = "..." }  # 只要扩展管理
ironclaw-safety = { path = "..." }      # 只要安全过滤
# 不需要 agent, llm, tools, sandbox 等
```

但这是一个大规模重构（预估 2-3 个月），需要 IronClaw 团队配合。

---

## 八、总结

| 问题 | 答案 |
|------|------|
| 能按需引入模块吗？ | 管理类可以，AI 对话不行（模块耦合太紧） |
| Sidecar 数据怎么上报？ | 企业场景不应该用 Sidecar，应该连中心 Server |
| 正确的企业架构？ | Client → 企业 IronClaw Server ← Admin Backend |
| Sidecar 适合什么场景？ | 个人用户、离线使用、不需要集中管理 |
| 长期怎么支持按需引入？ | IronClaw 拆分为多个独立 crate（大工程） |

> **核心结论**：你的项目有管理端（Admin Backend）、DLP、审计日志、RBAC，这是一个企业级产品。企业级产品的正确架构是中心化的 IronClaw Server + 轻量客户端，而不是每个客户端 Sidecar 一个完整的 IronClaw。Sidecar 方案只适合个人用户场景。
