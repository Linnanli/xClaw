# IronClaw 共享能力详解：Skills / Extensions / Routines / Settings / Logs

> 生成日期：2026-03-21
> 目的：详细解释 IronClaw 中 5 个需要通过 HTTP 代理访问的共享能力，包括它们是什么、怎么工作、数据结构、API 接口、以及管理端如何使用。

---

## 一、Skills（技能系统）

### 1.1 是什么？

Skills 是 IronClaw 的 **AI 提示词扩展系统**。简单说，就是给 AI 代理"加技能点"——通过加载额外的提示词文件，让 AI 在特定领域表现更好。

**类比**：如果 IronClaw 是一个员工，Skills 就是他的"培训手册"。安装一个 "Python Expert" 技能，AI 在写 Python 代码时就会参考这个手册里的最佳实践。

### 1.2 怎么工作？

```
用户安装技能 → SKILL.md 文件写入磁盘 → SkillRegistry 扫描并加载 → AI 对话时根据关键词匹配激活
```

**存储方式**：纯文件系统，不是数据库。

- 用户级技能：`~/.ironclaw/skills/` 目录
- 工作区级技能：`workspace/skills/` 目录
- 已安装技能（从注册表安装）：`~/.ironclaw/installed_skills/` 目录

**文件格式**：每个技能是一个 `SKILL.md` 文件，包含 YAML 前置元数据 + Markdown 提示词正文。

```markdown
---
name: python-expert
description: Python 编程最佳实践指导
version: 1.0.0
keywords:
  - python
  - django
  - flask
activation:
  always: false
  keywords: ["python", "django", "flask", "pip"]
---

# Python Expert

你是一个 Python 专家。在编写 Python 代码时，请遵循以下原则：
1. 使用类型注解
2. 遵循 PEP 8 规范
3. 优先使用 pathlib 而非 os.path
...
```

### 1.3 核心数据结构

```rust
// 技能清单（从 YAML 前置元数据解析）
struct SkillManifest {
    name: String,           // 技能名称，如 "python-expert"
    description: String,    // 技能描述
    version: String,        // 版本号，默认 "1.0.0"
    keywords: Vec<String>,  // 关键词列表
    activation: ActivationCriteria, // 激活条件
}

// 激活条件
struct ActivationCriteria {
    always: bool,              // 是否始终激活
    keywords: Vec<String>,     // 触发关键词（最多 20 个）
    patterns: Vec<String>,     // 正则匹配模式（最多 5 个）
    max_context_tokens: usize, // 最大上下文 token 数，默认 8000
}

// 已加载的技能
struct LoadedSkill {
    manifest: SkillManifest,
    content: String,        // 完整的 Markdown 提示词内容
    trust: SkillTrust,      // 信任级别
    source: SkillSource,    // 来源
    path: PathBuf,          // 文件路径
    hash: String,           // 内容 SHA256 哈希
}

// 信任级别（决定 AI 能使用哪些工具）
enum SkillTrust {
    Installed,  // 从注册表安装 → 只能用只读工具
    Trusted,    // 用户本地创建 → 可以用所有工具
}

// 来源
enum SkillSource {
    User,       // 用户目录 (~/.ironclaw/skills/)
    Workspace,  // 工作区目录 (workspace/skills/)
    Installed,  // 注册表安装 (~/.ironclaw/installed_skills/)
}
```

### 1.4 API 接口

| 方法 | 路径 | 说明 | 请求体 | 响应 |
|------|------|------|--------|------|
| GET | `/api/skills` | 列出所有已安装技能 | 无 | `{ skills: [{ name, description, version, trust, source, keywords, activation }] }` |
| POST | `/api/skills/search` | 搜索技能（本地 + ClawHub 注册表） | `{ query: "python" }` | `{ results: [{ name, description, version, source }] }` |
| POST | `/api/skills/install` | 安装技能（需确认头） | `{ name: "python-expert" }` | `{ status: "installed" }` |
| DELETE | `/api/skills/{name}` | 移除技能 | 无 | `{ status: "removed" }` |

### 1.5 管理端怎么用？

管理端通过 HTTP 代理调用 IronClaw 的 Skills API，主要用于：
- **查看**：列出当前 IronClaw 实例安装了哪些技能
- **审计**：记录技能的安装/卸载操作
- **远程管理**（可选）：通过管理端安装/卸载技能

**为什么不能自己实现**：技能文件存储在 IronClaw 运行的机器的文件系统上，管理端无法直接访问。

---

## 二、Extensions（扩展系统）

### 2.1 是什么？

Extensions 是 IronClaw 的 **工具和通道扩展系统**。它让 AI 代理能够连接外部服务、使用第三方工具。

**类比**：如果 Skills 是"培训手册"，Extensions 就是"工具箱"。安装一个 GitHub 扩展，AI 就能直接操作 GitHub 仓库；安装一个 Slack 扩展，AI 就能在 Slack 里回复消息。

### 2.2 四种扩展类型

```
┌─────────────────────────────────────────────────────────────────┐
│                    Extensions 四种类型                            │
├─────────────────┬───────────────────────────────────────────────┤
│ MCP Server      │ 托管的 MCP 协议服务器                           │
│                 │ - HTTP 传输，OAuth 2.1 认证                     │
│                 │ - 例如：GitHub MCP、Jira MCP、数据库 MCP         │
│                 │ - 提供工具（tools）给 AI 调用                    │
├─────────────────┼───────────────────────────────────────────────┤
│ WASM Tool       │ WebAssembly 沙箱工具                            │
│                 │ - 下载 .wasm 文件，在 wasmtime 沙箱中执行        │
│                 │ - 安全隔离，有能力认证机制                       │
│                 │ - 例如：代码格式化工具、数据转换工具              │
├─────────────────┼───────────────────────────────────────────────┤
│ WASM Channel    │ WebAssembly 通道模块                            │
│                 │ - 热激活支持，运行时状态管理                     │
│                 │ - 例如：Telegram 通道（通过 WASM 实现）          │
│                 │ - 需要所有者绑定验证                             │
├─────────────────┼───────────────────────────────────────────────┤
│ Channel Relay   │ 外部通道中继                                    │
│                 │ - 通过 channel-relay 外部服务转发                │
│                 │ - 例如：Slack 通道                               │
│                 │ - 需要 OAuth 认证流程                            │
└─────────────────┴───────────────────────────────────────────────┘
```

### 2.3 生命周期

```
发现（Discovery）→ 安装（Install）→ 认证（Authenticate）→ 激活（Activate）→ 使用
     │                  │                  │                    │
     │                  │                  │                    │
  从注册表或        下载到本地         OAuth 流程或          启动运行时
  在线发现          或配置连接         API Key 配置         （MCP 连接/WASM 编译）
```

### 2.4 核心数据结构

```rust
// 扩展类型
enum ExtensionKind {
    McpServer,      // MCP 协议服务器
    WasmTool,       // WASM 沙箱工具
    WasmChannel,    // WASM 通道模块
    ChannelRelay,   // 外部通道中继
}

// 已安装的扩展
struct InstalledExtension {
    name: String,              // 扩展名称，如 "github"
    display_name: String,      // 显示名称，如 "GitHub"
    kind: ExtensionKind,       // 扩展类型
    description: String,       // 描述
    authenticated: bool,       // 是否已认证
    active: bool,              // 是否已激活（运行中）
    tools: Vec<String>,        // 提供的工具列表
    needs_setup: bool,         // 是否需要初始设置
    has_auth: bool,            // 是否需要认证
    activation_status: String, // 激活状态描述
    version: Option<String>,   // 版本号
    // ... 还有更多字段
}

// 注册表条目（已知的可安装扩展）
struct RegistryEntry {
    name: String,
    display_name: String,
    kind: ExtensionKind,
    description: String,
    source: ExtensionSource,   // 安装来源（URL/文件路径等）
    auth_hint: Option<AuthHint>, // 认证提示
}

// 认证结果
struct AuthResult {
    name: String,
    kind: ExtensionKind,
    status: AuthStatus,        // Authenticated / AwaitingAuthorization / NeedsSetup 等
    auth_url: Option<String>,  // OAuth 授权 URL
    instructions: Option<String>, // 设置说明
    // ...
}
```

### 2.5 API 接口

| 方法 | 路径 | 说明 |
|------|------|------|
| GET | `/api/extensions` | 列出所有已安装扩展（含状态、工具列表） |
| GET | `/api/extensions/tools` | 列出所有可用工具 |
| GET | `/api/extensions/registry` | 列出注册表中的可安装扩展 |
| POST | `/api/extensions/install` | 安装扩展 |
| POST | `/api/extensions/{name}/activate` | 激活扩展 |
| POST | `/api/extensions/{name}/remove` | 移除扩展 |
| GET | `/api/extensions/{name}/setup` | 获取扩展设置表单 |
| POST | `/api/extensions/{name}/setup` | 提交扩展设置 |

### 2.6 管理端怎么用？

管理端通过 HTTP 代理调用 IronClaw 的 Extensions API，主要用于：
- **查看**：列出已安装扩展、类型、认证状态、激活状态、工具列表
- **监控**：哪些扩展在运行、哪些需要重新认证
- **审计**：记录扩展的安装/卸载/激活操作

**为什么不能自己实现**：
- ExtensionManager 有 7000+ 行代码，涉及 OAuth 2.1、WASM 编译、Docker 沙箱
- 扩展的运行时状态（连接、进程、WASM 实例）在 IronClaw 进程内存中
- 管理端只需要"看"，不需要"跑"

---

## 三、Routines（例程/自动化任务系统）

### 3.1 是什么？

Routines 是 IronClaw 的 **自动化任务系统**。它让 AI 代理能够按计划或按事件自动执行任务。

**类比**：Routines 就像 crontab + IFTTT 的结合体。你可以设置"每天早上 9 点总结昨天的 GitHub Issues"，或者"当收到包含'紧急'的消息时自动分析并回复"。

### 3.2 触发方式

```
┌─────────────────────────────────────────────────────────────────┐
│                    Routines 四种触发方式                          │
├─────────────────┬───────────────────────────────────────────────┤
│ Cron（定时）     │ 基于 cron 表达式定时触发                       │
│                 │ - 支持时区设置                                  │
│                 │ - 例如："0 9 * * MON-FRI"（工作日早9点）         │
│                 │ - 例如："every 2h"（每2小时）                   │
├─────────────────┼───────────────────────────────────────────────┤
│ Event（事件）    │ 当通道消息匹配正则模式时触发                    │
│                 │ - 可选通道过滤（telegram/slack 等）              │
│                 │ - 例如：pattern="紧急|urgent"                   │
├─────────────────┼───────────────────────────────────────────────┤
│ SystemEvent     │ 当系统事件发生时触发                            │
│ （系统事件）     │ - source: "github", event_type: "issue.opened" │
│                 │ - 支持 payload 字段精确匹配过滤                 │
├─────────────────┼───────────────────────────────────────────────┤
│ Manual（手动）   │ 只能通过 API 调用或 CLI 手动触发                │
│                 │ - 适合一次性任务或测试                          │
└─────────────────┴───────────────────────────────────────────────┘
```

### 3.3 执行方式

```
┌─────────────────────────────────────────────────────────────────┐
│                    Routines 两种执行方式                          │
├─────────────────┬───────────────────────────────────────────────┤
│ Lightweight     │ 轻量级执行                                     │
│ （轻量模式）     │ - 直接调用 LLM，可选工具调用                    │
│                 │ - 适合简单任务（总结、分析、通知）               │
│                 │ - 可配置 max_tokens、max_iterations             │
│                 │ - 支持工具白名单/黑名单                         │
├─────────────────┼───────────────────────────────────────────────┤
│ FullJob         │ 完整任务执行                                    │
│ （完整模式）     │ - 创建一个完整的 Job（任务）                    │
│                 │ - 有独立的工作区和上下文                         │
│                 │ - 适合复杂任务（代码修改、多步骤操作）           │
│                 │ - 可以等待任务完成后获取结果                     │
└─────────────────┴───────────────────────────────────────────────┘
```

### 3.4 核心数据结构

```rust
// 例程
struct Routine {
    id: Uuid,                          // 唯一 ID
    name: String,                      // 名称，如 "daily-github-summary"
    description: String,               // 描述
    user_id: String,                   // 所属用户
    enabled: bool,                     // 是否启用
    trigger: Trigger,                  // 触发条件（Cron/Event/SystemEvent/Manual）
    action: RoutineAction,             // 执行方式（Lightweight/FullJob）
    guardrails: RoutineGuardrails,     // 安全护栏
    notify: NotifyConfig,              // 通知配置

    // 运行时状态（数据库管理）
    last_run_at: Option<DateTime>,     // 上次运行时间
    next_fire_at: Option<DateTime>,    // 下次触发时间
    run_count: u64,                    // 总运行次数
    consecutive_failures: u32,         // 连续失败次数
    state: serde_json::Value,          // 自定义状态数据

    created_at: DateTime,
    updated_at: DateTime,
}

// 安全护栏
struct RoutineGuardrails {
    require_approval: bool,            // 是否需要人工审批
    approval_patterns: Vec<String>,    // 需要审批的操作模式
    max_tokens: u32,                   // 最大 token 消耗
    max_iterations: u32,               // 最大迭代次数
    max_tool_rounds: u32,              // 最大工具调用轮次（上限 20）
    tool_permissions: Vec<String>,     // 工具白名单
}

// 通知配置
struct NotifyConfig {
    on_success: bool,                  // 成功时通知
    on_failure: bool,                  // 失败时通知
    attention_notifications: bool,     // 需要关注时通知
    channel: Option<String>,           // 通知通道
}

// 运行记录
struct RoutineRun {
    id: Uuid,
    routine_id: Uuid,
    trigger_type: String,              // 触发类型
    started_at: DateTime,
    completed_at: Option<DateTime>,
    status: RunStatus,                 // Running/Completed/Failed/Cancelled/TimedOut
    result_summary: Option<String>,    // 结果摘要
    tokens_used: Option<i32>,          // 消耗的 token 数
    job_id: Option<Uuid>,              // 关联的 Job ID（FullJob 模式）
}
```

### 3.5 API 接口

| 方法 | 路径 | 说明 | 响应关键字段 |
|------|------|------|-------------|
| GET | `/api/routines` | 列出所有例程 | `{ routines: [{ id, name, description, enabled, trigger_type, trigger_summary, last_run_at, next_fire_at, run_count, consecutive_failures }] }` |
| GET | `/api/routines/summary` | 例程统计摘要 | `{ total, enabled, disabled, failing, runs_today }` |
| GET | `/api/routines/{id}` | 例程详情（含最近 20 条运行记录） | `{ id, name, trigger, action, guardrails, notify, recent_runs: [...] }` |
| POST | `/api/routines/{id}/trigger` | 手动触发例程 | `{ status: "triggered", run_id }` |
| POST | `/api/routines/{id}/toggle` | 启用/禁用例程 | `{ status: "enabled"/"disabled" }` |
| DELETE | `/api/routines/{id}` | 删除例程 | `{ status: "deleted" }` |
| GET | `/api/routines/{id}/runs` | 查看运行历史（最近 50 条） | `{ runs: [{ id, trigger_type, started_at, status, result_summary, tokens_used }] }` |

### 3.6 管理端怎么用？

管理端通过 HTTP 代理调用 IronClaw 的 Routines API，主要用于：
- **监控**：查看所有例程的运行状态、成功/失败率、下次触发时间
- **控制**：启用/禁用例程、手动触发、删除
- **审计**：查看运行历史、token 消耗、失败原因
- **统计**：总例程数、今日运行数、失败数

**为什么不能自己实现**：
- RoutineEngine 依赖 LLM Provider、ToolRegistry、Workspace、SafetyLayer 等核心组件
- 例程的执行需要 AI 推理能力，管理端没有 LLM
- 例程的运行时状态（定时器、事件缓存、并发控制）在 IronClaw 进程内存中

---

## 四、Settings（设置系统）

### 4.1 是什么？

Settings 是 IronClaw 的 **运行时配置系统**。它管理 AI 代理运行所需的所有配置项，从 LLM 模型选择到沙箱参数。

**类比**：Settings 就像一个应用的"偏好设置"面板。你可以在这里切换 AI 模型、调整安全参数、配置通道连接等。

### 4.2 配置优先级

```
环境变量 (.env)  >  TOML 配置文件  >  数据库存储  >  代码默认值
   最高优先级          中优先级          中优先级        最低优先级
```

### 4.3 配置分类（~100 个配置项）

```
┌─────────────────────────────────────────────────────────────────────┐
│                    Settings 配置分类                                  │
├──────────────────┬──────────────────────────────────────────────────┤
│ 基础配置          │ onboard_completed: 是否完成初始化向导              │
│                  │ owner_id: 实例所有者 ID                           │
├──────────────────┼──────────────────────────────────────────────────┤
│ 数据库配置        │ database_backend: "postgres" 或 "libsql"          │
│ (Step 1)         │ database_url: 连接字符串                          │
│                  │ database_pool_size: 连接池大小                     │
│                  │ libsql_path: 本地 libSQL 文件路径                  │
│                  │ libsql_url: Turso 云端 URL                        │
├──────────────────┼──────────────────────────────────────────────────┤
│ 安全配置          │ secrets_master_key_source: 密钥来源               │
│ (Step 2)         │ secrets_master_key_hex: 主密钥（仅 env 模式）      │
├──────────────────┼──────────────────────────────────────────────────┤
│ LLM 推理配置      │ llm_backend: "anthropic"/"openai"/"ollama"/       │
│ (Step 3)         │   "bedrock"/"openai_compatible"/"tinfoil"/"nearai" │
│                  │ ollama_base_url: Ollama 地址                      │
│                  │ openai_compatible_base_url: 兼容端点地址           │
│                  │ bedrock_region: AWS 区域                          │
│                  │ bedrock_cross_region: 跨区域推理前缀               │
│                  │ bedrock_profile: AWS Profile                      │
├──────────────────┼──────────────────────────────────────────────────┤
│ 模型选择          │ selected_model: 当前使用的模型名称                 │
│ (Step 4)         │                                                  │
├──────────────────┼──────────────────────────────────────────────────┤
│ 嵌入向量配置      │ embeddings.provider: 嵌入向量提供商               │
│ (Step 5)         │ embeddings.model: 嵌入模型名称                    │
│                  │ embeddings.api_key: API 密钥                      │
│                  │ embeddings.base_url: 自定义端点                    │
├──────────────────┼──────────────────────────────────────────────────┤
│ 通道配置          │ channels.telegram_token: Telegram Bot Token       │
│ (Step 6)         │ channels.slack_*: Slack 配置                      │
│                  │ channels.discord_*: Discord 配置                   │
│                  │ channels.web_enabled: 是否启用 Web 通道            │
│                  │ tunnel.*: 隧道配置（公网 webhook）                 │
├──────────────────┼──────────────────────────────────────────────────┤
│ 心跳配置          │ heartbeat.interval: 心跳间隔（秒）                │
│ (Step 7)         │ heartbeat.enabled: 是否启用                       │
├──────────────────┼──────────────────────────────────────────────────┤
│ Agent 行为配置    │ agent.name: AI 代理名称                           │
│ (高级)           │ agent.max_parallel_jobs: 最大并行任务数             │
│                  │ agent.job_timeout: 任务超时（秒）                  │
│                  │ agent.stuck_threshold: 卡住检测阈值                │
│                  │ agent.repair_interval: 自修复间隔                  │
│                  │ agent.session_idle_timeout: 会话空闲超时            │
│                  │ agent.max_tool_iterations: 最大工具迭代次数         │
│                  │ agent.timezone: 时区                               │
│                  │ agent.auto_approve_tools: 自动审批工具              │
├──────────────────┼──────────────────────────────────────────────────┤
│ WASM 沙箱配置     │ wasm.memory_limit: 内存限制（字节）               │
│ (高级)           │ wasm.timeout: 执行超时（秒）                      │
│                  │ wasm.fuel_limit: 燃料限制                         │
├──────────────────┼──────────────────────────────────────────────────┤
│ Docker 沙箱配置   │ sandbox.policy: 沙箱策略                         │
│ (高级)           │ sandbox.timeout: 超时（秒）                       │
│                  │ sandbox.memory: 内存限制                          │
│                  │ sandbox.cpu_shares: CPU 份额                      │
│                  │ sandbox.image: Docker 镜像                        │
├──────────────────┼──────────────────────────────────────────────────┤
│ 安全层配置        │ safety.max_output_length: 最大输出长度             │
│ (高级)           │                                                  │
├──────────────────┼──────────────────────────────────────────────────┤
│ Builder 配置      │ builder.max_iterations: 最大构建迭代次数           │
│ (高级)           │ builder.timeout: 构建超时（秒）                    │
├──────────────────┼──────────────────────────────────────────────────┤
│ 转录配置          │ transcription.provider: 语音转文字提供商           │
│ (高级)           │ transcription.model: 转录模型                     │
└──────────────────┴──────────────────────────────────────────────────┘
```

### 4.4 存储方式

Settings 使用数据库的 key-value 存储，按 `user_id` 隔离：

```sql
-- IronClaw 的 settings 表
CREATE TABLE settings (
    user_id TEXT NOT NULL,
    key TEXT NOT NULL,
    value TEXT NOT NULL,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (user_id, key)
);
```

Settings 结构体支持双向转换：
- `to_db_map()` → 将结构体序列化为 `HashMap<String, Value>`，写入数据库
- `from_db_map()` → 从数据库读取并反序列化为结构体
- `load_toml()` / `save_toml()` → TOML 文件读写
- `get(path)` / `set(path, value)` → 按路径读写单个配置项

### 4.5 API 接口

| 方法 | 路径 | 说明 | 示例 |
|------|------|------|------|
| GET | `/api/settings` | 列出所有设置 | `{ settings: [{ key: "selected_model", value: "claude-3.5-sonnet", updated_at: "..." }] }` |
| GET | `/api/settings/{key}` | 获取单个设置 | `{ key: "llm_backend", value: "anthropic", updated_at: "..." }` |
| PUT | `/api/settings/{key}` | 修改设置 | 请求体：`{ value: "openai" }` |
| DELETE | `/api/settings/{key}` | 删除设置（恢复默认值） | 204 No Content |
| GET | `/api/settings/export` | 导出所有设置 | `{ settings: { "key1": "value1", ... } }` |
| POST | `/api/settings/import` | 导入设置 | 请求体：`{ settings: { "key1": "value1", ... } }` |

### 4.6 管理端怎么用？

管理端通过 HTTP 代理调用 IronClaw 的 Settings API，主要用于：
- **查看**：当前 IronClaw 使用的 LLM 模型、数据库配置、通道配置等
- **修改**：远程切换 LLM 模型、调整安全参数、配置通道
- **导入/导出**：批量配置管理，方便多实例部署

**为什么不能自己实现**：
- Settings 写入的是 IronClaw 的数据库，管理端有自己独立的数据库
- 修改 Settings 后 IronClaw 需要实时生效（如切换 LLM 模型），这需要 IronClaw 进程内的状态更新
- 管理端自己实现的话，配置和 IronClaw 实际使用的会不同步

**注意**：管理端自己的 `system_settings` 表存储的是安全管理平台的配置（DLP、审计、客户端管理等），和 IronClaw 的 Settings 完全不同，两者不冲突。

---

## 五、Logs（日志系统）

### 5.1 是什么？

Logs 是 IronClaw 的 **实时日志流系统**。它通过 SSE（Server-Sent Events）将 IronClaw 的运行日志实时推送给订阅者。

**类比**：就像 `tail -f` 命令，但通过浏览器实时查看。管理端可以像看监控大屏一样实时观察 IronClaw 的运行状态。

### 5.2 架构

```
IronClaw 进程内部：

  tracing 日志框架
       │
       ▼
  WebLogLayer（自定义 tracing Layer）
       │
       ▼
  LogBroadcaster.send()  ←── 所有日志的唯一出口
       │
       ├──► broadcast::Sender  ──→ 实时推送给所有 SSE 订阅者
       │
       ├──► VecDeque<LogEntry>  ──→ 环形缓冲区（最近 200 条）
       │                            新连接的浏览器可以看到启动日志
       │
       └──► LeakDetector.scrub()  ──→ 自动脱敏（API Key 等敏感信息）
```

### 5.3 核心数据结构

```rust
// 日志条目
struct LogEntry {
    timestamp: String,     // ISO 8601 时间戳
    level: String,         // "TRACE"/"DEBUG"/"INFO"/"WARN"/"ERROR"
    target: String,        // 模块路径，如 "ironclaw::agent"
    message: String,       // 日志消息（已脱敏）
}

// 日志广播器
struct LogBroadcaster {
    tx: broadcast::Sender<LogEntry>,       // 广播通道（容量 512）
    recent: Mutex<VecDeque<LogEntry>>,     // 环形缓冲区（最近 200 条）
    leak_detector: LeakDetector,           // 敏感信息脱敏器
}

// 日志级别控制器
struct LogLevelHandle {
    reload_handle: Handle<EnvFilter, ...>, // 运行时可重载的日志过滤器
    current_filter: Mutex<String>,         // 当前过滤规则
}
```

### 5.4 关键特性

1. **实时推送**：通过 SSE 协议，日志产生后立即推送，延迟 <10ms
2. **历史回放**：新连接的客户端会先收到最近 200 条历史日志，再接收实时日志
3. **自动脱敏**：所有日志经过 `LeakDetector` 处理，API Key、密码等敏感信息自动替换为 `[REDACTED]`
4. **运行时级别调整**：可以通过 API 动态调整日志级别（如从 INFO 切换到 DEBUG），无需重启
5. **Keep-Alive**：SSE 连接每 30 秒发送心跳，防止连接超时断开

### 5.5 API 接口

| 方法 | 路径 | 说明 | 响应格式 |
|------|------|------|---------|
| GET | `/api/logs/events` | SSE 日志流（实时 + 历史） | SSE 事件流，每条日志是一个 `event: log` 事件 |
| GET | `/api/logs/level` | 获取当前日志级别 | `{ level: "ironclaw=info,tower_http=warn" }` |
| PUT | `/api/logs/level` | 设置日志级别 | 请求体：`{ level: "ironclaw=debug" }` |

**SSE 事件格式**：
```
event: log
data: {"timestamp":"2026-03-21T10:30:00Z","level":"INFO","target":"ironclaw::agent","message":"Processing chat message..."}

event: log
data: {"timestamp":"2026-03-21T10:30:01Z","level":"DEBUG","target":"ironclaw::tools","message":"Executing tool: read_file"}
```

### 5.6 管理端怎么用？

管理端通过 HTTP 代理（SSE 转发）调用 IronClaw 的 Logs API，主要用于：
- **实时监控**：在管理面板中实时查看 IronClaw 的运行日志
- **问题排查**：当 AI 代理行为异常时，查看详细日志定位问题
- **级别调整**：远程调整日志级别，临时开启 DEBUG 模式排查问题
- **安全审计**：监控是否有异常操作或安全事件

**为什么不能自己实现**：
- 日志是 IronClaw 进程内部产生的，管理端无法自己生成
- `LogBroadcaster` 是进程内的广播通道，只能通过 SSE 订阅
- 这是唯一一个**物理上不可能自己实现**的能力

---

## 六、五个能力的对比总结

### 6.1 能力定位对比

| 能力 | 一句话定义 | 类比 | 复杂度 |
|------|-----------|------|--------|
| Skills | AI 的"培训手册"扩展 | 给员工发培训资料 | ⭐⭐ 中 |
| Extensions | AI 的"工具箱"扩展 | 给员工配工具和通讯设备 | ⭐⭐⭐⭐⭐ 极高 |
| Routines | AI 的"自动化任务"调度 | 给员工排班表和自动化流程 | ⭐⭐⭐⭐ 高 |
| Settings | AI 的"偏好设置"面板 | 调整员工的工作参数 | ⭐⭐ 中 |
| Logs | AI 的"工作日志"实时流 | 实时查看员工的工作记录 | ⭐ 低 |

### 6.2 数据存储对比

| 能力 | 存储位置 | 存储方式 | 管理端能直接访问？ |
|------|---------|---------|------------------|
| Skills | IronClaw 机器的文件系统 | SKILL.md 文件 | ❌ 不能 |
| Extensions | IronClaw 进程内存 + 文件系统 | 运行时状态 + 配置文件 | ❌ 不能 |
| Routines | IronClaw 的数据库 + 进程内存 | DB 表 + 内存缓存 | ❌ 不能（不同数据库） |
| Settings | IronClaw 的数据库 + TOML 文件 | key-value 表 + 配置文件 | ❌ 不能（不同数据库） |
| Logs | IronClaw 进程内存 | 广播通道 + 环形缓冲区 | ❌ 不能 |

### 6.3 管理端操作权限对比

| 能力 | 只读操作 | 写操作 | 控制操作 |
|------|---------|--------|---------|
| Skills | ✅ 列表、详情 | ⚠️ 安装/卸载（通过代理） | 无 |
| Extensions | ✅ 列表、状态、工具 | ⚠️ 安装/卸载（通过代理） | ⚠️ 激活/停用 |
| Routines | ✅ 列表、详情、运行历史 | ❌ 创建（由用户在客户端创建） | ✅ 启用/禁用、手动触发、删除 |
| Settings | ✅ 列表、单项查询 | ✅ 修改、导入 | ✅ 重置、导出 |
| Logs | ✅ 实时流、历史 | 无 | ✅ 调整日志级别 |

### 6.4 代码量和依赖对比

| 能力 | IronClaw 代码量 | 核心依赖 | 管理端需要的代码量 |
|------|----------------|---------|------------------|
| Skills | ~1200 行（registry + parser） | serde_yml, regex, sha2 | ~50 行（HTTP 代理） |
| Extensions | ~7000+ 行（manager） | wasmtime, bollard, OAuth | ~50 行（HTTP 代理） |
| Routines | ~1700 行（engine + routine） | cron, tokio, LLM | ~80 行（HTTP 代理） |
| Settings | ~2300 行（settings + handlers） | serde, toml | ~60 行（HTTP 代理） |
| Logs | ~300 行（broadcaster + layer） | tracing, broadcast | ~30 行（SSE 转发） |

### 6.5 管理端已实现 vs 待实现

| 能力 | 当前状态 | 待优化 |
|------|---------|--------|
| Skills | ✅ 已有代理（`/api/skills`） | 删除冗余 DB 表，完善字段映射 |
| Extensions | ✅ 已有代理（`/api/plugins`） | 删除冗余 DB 表，展示更多字段（kind, tools, auth 状态） |
| Routines | ❌ 未实现 | 新增代理端点（列表、详情、触发、启停） |
| Settings | ✅ 已有代理（`/api/settings`） | 区分 IronClaw Settings 和管理端 Settings |
| Logs | ❌ 未实现 | 新增 SSE 转发端点 |

---

## 七、管理端集成建议

### 7.1 短期优化（现有能力）

```
Skills 代理优化：
  - 删除本地 skills 表
  - 透传 IronClaw 完整响应（trust, source, keywords, activation）
  - 前端展示信任级别标签和来源标签

Extensions 代理优化：
  - 删除本地 plugins 表
  - 透传 IronClaw 完整响应（kind, authenticated, active, tools, needs_setup）
  - 前端展示扩展类型、认证状态、工具列表
```

### 7.2 新增代理（Routines + Logs）

```
Routines 代理（新增）：
  GET  /api/gateway/routines          → IronClaw GET /api/routines
  GET  /api/gateway/routines/summary  → IronClaw GET /api/routines/summary
  GET  /api/gateway/routines/:id      → IronClaw GET /api/routines/:id
  POST /api/gateway/routines/:id/trigger → IronClaw POST /api/routines/:id/trigger
  POST /api/gateway/routines/:id/toggle  → IronClaw POST /api/routines/:id/toggle
  GET  /api/gateway/routines/:id/runs → IronClaw GET /api/routines/:id/runs

Logs 代理（新增）：
  GET  /api/gateway/logs/events       → IronClaw GET /api/logs/events（SSE 转发）
  GET  /api/gateway/logs/level        → IronClaw GET /api/logs/level
  PUT  /api/gateway/logs/level        → IronClaw PUT /api/logs/level
```

### 7.3 URL 命名建议

建议将所有 IronClaw 代理端点统一放在 `/api/gateway/` 前缀下，与管理端自有的 API 区分：

```
管理端自有 API：
  /api/auth/*           → 认证
  /api/users/*          → 用户管理
  /api/dlp-rules/*      → DLP 规则
  /api/dictionaries/*   → 字典管理
  /api/sensitive-ops/*  → 敏感操作
  /api/clients/*        → 客户端管理
  /api/audit-logs/*     → 审计日志
  /api/settings/*       → 管理端自己的设置

IronClaw 代理 API：
  /api/gateway/skills/*     → 技能管理（代理）
  /api/gateway/extensions/* → 扩展管理（代理）
  /api/gateway/routines/*   → 例程管理（代理）
  /api/gateway/settings/*   → IronClaw 设置（代理）
  /api/gateway/logs/*       → 日志监控（代理）
  /api/gateway/health       → Gateway 健康检查
```

---

## 八、参考文件

| 文件 | 说明 |
|------|------|
| `ironclaw/src/skills/mod.rs` | Skills 类型定义（SkillManifest, LoadedSkill, SkillTrust） |
| `ironclaw/src/skills/registry.rs` | SkillRegistry 实现（~1200 行） |
| `ironclaw/src/extensions/mod.rs` | Extensions 类型定义（ExtensionKind, InstalledExtension） |
| `ironclaw/src/extensions/manager.rs` | ExtensionManager 实现（~7000 行） |
| `ironclaw/src/agent/routine.rs` | Routine 类型定义（Trigger, RoutineAction, RoutineGuardrails） |
| `ironclaw/src/agent/routine_engine.rs` | RoutineEngine 实现（~1700 行） |
| `ironclaw/src/settings.rs` | Settings 结构体（~100 个配置项，~2300 行） |
| `ironclaw/src/channels/web/log_layer.rs` | LogBroadcaster + WebLogLayer（~300 行） |
| `ironclaw/src/channels/web/handlers/skills.rs` | Skills API handlers |
| `ironclaw/src/channels/web/handlers/extensions.rs` | Extensions API handlers |
| `ironclaw/src/channels/web/handlers/routines.rs` | Routines API handlers |
| `ironclaw/src/channels/web/handlers/settings.rs` | Settings API handlers |
| `ironclaw/src/channels/web/server.rs` | Logs API handlers（内联在 server.rs 中） |
| `ironclaw/src/db/mod.rs` | RoutineStore trait（数据库接口） |
