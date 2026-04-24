# 14 — Claude Code 能力对账 (Route B 缺口全扫描)

> **产出时机**: Round 17 (2025-01, 架构重构迭代对话)
> **触发原因**: 前 15 轮分析仅用字面量搜索 (`rg`), 漏掉 codex `SpawnAgentForkMode`、误判"ironclaw Prompt Cache 独家"、未覆盖 Claude Code 启动优化/Feature Flag/高级压缩等能力。本 doc 做系统对账, 避免再次漏判。
> **方法论**: 以 `decode-claude-code-main/` 12 章为"能力全集", 对每章列 Claude Code 实现 → 三路 (codex / claw-code / ironclaw) 有无 → Route B 决策 → 优先级。

---

## 0. 工具使用原则 (从本次分析教训沉淀)

### 0.1 错误归因

Round 1-15 的三次误判全部来自**字面量搜索陷阱**:

| 误判 | 根因 | 正确工具 |
|------|------|---------|
| "codex 没 forkSubagent" | 搜 `forkSubagent` 字面量, 漏 codex 命名 `SpawnAgentForkMode` | `semantic_search` 搜 "agent fork parent history" |
| "ironclaw Prompt Cache 独家" | 只读 175 行 `prompt_cache.rs`, 没看到源码注释 "injected through claw-code-api" | `vscode_listCodeUsages` 追踪 `cache_control` 符号 |
| "架构文档已覆盖压缩/权限/缓存等核心 5 件" | 只对齐 codex vs ironclaw vs claw-code 三方, 未对齐 Claude Code 原版能力 | 系统读 decode-claude-code 12 章 |

### 0.2 三级工具使用规范 (团队约定)

**重大架构决策前必须执行**:

```
Level 1 语义层    →  semantic_search           (向量搜索, 跨命名概念)
Level 2 符号层    →  vscode_listCodeUsages     (LSP 引用/定义/实现图)
Level 3 字面量层  →  rg / grep                  (已知确切词后再用)
```

**反模式** (禁止):
- ❌ 跳过 Level 1, 直接 `rg <英文词>` 找概念 — 漏掉异名等价实现
- ❌ 不读解构文档, 只看代码 — 漏掉未 port 的原版能力
- ❌ 单一 repo 判定 "独家" — 必须三方交叉验证

### 0.3 能力对账表结构 (标准化)

对任意架构能力, 应填充以下表格再做决策:

| 字段 | 说明 |
|------|------|
| Claude Code 实现 | 源码路径 + 行数 + Feature Flag 状态 |
| codex 实现 | 有/无 + 源码路径 + 等价命名 |
| claw-code 实现 | 有/无 + 源码路径 + port 状态 |
| ironclaw 实现 | 有/无 + 源码路径 + 依赖其他方情况 |
| Route B 决策 | 复用谁 / port 谁 / 新建 crate |
| 优先级 | P0-P3 |
| 工作量估算 | 粗估 LOC |

---

## 1. 分章能力盘点

### Chapter 00 — 全局架构概览

**Claude Code 能力**: 17 层架构 (UI / Commands / Agent Loop / Tools / Permissions / Context / Cache / MCP / Multi-agent / ...)

**Route B 覆盖**: ✅ 我们的 5 层架构 (L1 channels / L2 context / L3 orchestration / L4 tools / L5 safety) 与之对应, 见 [11 doc §1](./11-target-architecture-route-b.md#1-总览-五层架构)。

**结论**: 架构分层基本对齐, 无缺口。

---

### Chapter 01 — System Prompt 组装

**Claude Code 能力**:
- 主 System Prompt 约 13K 字符
- 模板化构建 (`buildSystemPrompt`), 按上下文条件拼接
- 包含 `CYBER_RISK_INSTRUCTION` Anthropic 安全约束 (嵌入最前)
- `<system-reminder>` 标签机制区分系统/用户消息 (Prompt Injection 防御)

**三方映射**:

| 能力 | codex | claw-code | ironclaw | Route B |
|------|-------|-----------|---------|---------|
| 按 concern 拆分 (30+ `*_instructions.rs`) | ✅ 最完整 | ❌ | ❌ (散落多处) | ✅ 已归 codex, 新 crate `dasclaw_context_mgr` W4 |
| `CYBER_RISK_INSTRUCTION` 类安全约束 | ⚠️ 未确认 | ❌ | ❌ | 🔴 **P0 缺口**, port codex prompt + 补安全约束文本 |
| `<system-reminder>` 标签 | ⚠️ 未确认 | ❌ | ❌ | 🔴 **P0 缺口**, 补到 context 组装层 |

**缺口动作**:
1. **Round 18 验证**: `semantic_search` 搜 codex "cyber risk" / "system reminder" / "injection warning"
2. 若无, 新建 `dasclaw_prompt_safety` 或纳入 `dasclaw_context_mgr::safety_instructions`

---

### Chapter 02 — Agent Loop 核心循环

**Claude Code 能力**:
- `tt` 主循环 (generator 模式)
- 工具调用的流式处理
- 错误恢复 + 中断恢复

**Route B 覆盖**: ✅ 已规划 codex `core/agent/run_loop.rs` port → `dasclaw_agent_loop`, 见 [11 doc §4](./11-target-architecture-route-b.md#4-多-agent-协调架构).

**结论**: 覆盖, 无缺口。

---

### Chapter 03 — 工具系统

**Claude Code 能力**:
- BashTool (AST 安全分析 **7000+ 行**):
  - `bashParser.ts` 4,436 行 — AST 解析
  - `bashPermissions.ts` 2,621 行 — 权限判断
  - `bashSecurity.ts` 2,592 行 — 安全检查
  - `readOnlyValidation.ts` 1,990 行 — 只读模式验证
- FileTool / GrepTool / EditTool 等标准工具
- 工具注册 + 路由 + 并行

**三方映射**:

| 能力 | codex | claw-code | ironclaw | Route B |
|------|-------|-----------|---------|---------|
| 工具注册 + 路由 + 并行 | ✅ `core/tools/` 20 文件 | ❌ | ⚠️ `tools/` 加固层 | ✅ codex kernel port + ironclaw 加固, 见 §12.1 |
| **Bash AST 安全分析** | ❌ (只依赖 OS 沙箱) | ❌ | ❌ | 🔴 **P0 缺口**, 新 crate `dasclaw_bash_guard` ~5000+ 行 |
| sed/awk 修改标志识别 | ❌ | ❌ | ❌ | 🔴 **P0 缺口**, 归 `dasclaw_bash_guard` |
| 路径逃逸检测 (越 cwd) | ⚠️ sandbox 层有 | ⚠️ | ⚠️ | ⚠️ 依赖 OS 沙箱, 不做命令语义分析 |

**缺口详细说明**:

Claude Code 的 Bash 安全不是靠"沙箱拦截", 而是**执行前就通过 AST 解析判断命令语义**:

```
用户输入: `rm -rf /tmp/foo && cat secrets.txt`
Claude Code 路径:
  → bashParser 解析 AST
  → 识别 "rm -rf" 为破坏性 + "cat secrets.txt" 为读敏感文件
  → 分别触发不同审批策略
  → 经用户确认才执行

codex 路径:
  → seatbelt/landlock 直接放行 bash
  → 依赖进程隔离防护
  → 无法在确认对话框里提示 "这个命令会 rm -rf"
```

**Route B 决策**: 新 crate `dasclaw_bash_guard`, 基于 `tree-sitter-bash` 做 AST 解析, 移植 Claude Code 4 个 `.ts` 文件逻辑到 Rust。预计 3000-5000 LOC。

**归属**: 放在 L5 safety 层, 与 L4 tools 中的 BashTool 联动, 执行前先过 guard。

---

### Chapter 04 — 权限模型

**Claude Code 能力**:
- 5 种权限模式 (prompt / accept / bypass / plan / ignore)
- 工具级权限规则
- 永久允许列表

**Route B 覆盖**: ✅ [11 doc §12.1 第 3 行](./11-target-architecture-route-b.md#12-核心四项能力归属细化-round-12-审计) 已规划 `dasclaw_permissions` = codex 协议 + claw-code 引擎 + codex prompt/preset + ironclaw_safety L6。

**结论**: 覆盖, 无缺口。

---

### Chapter 05 — 上下文管理与压缩

**Claude Code 能力**:
- 200K token 窗口管理
- **75-92% 容量自动触发 `/compact`**
- **HISTORY_SNIP** (Feature Flag, 选择性剪切压缩, 保留结构)
- **CACHED_MICROCOMPACT** (Feature Flag, 缓存微压缩)
- **TOKEN_BUDGET** (Feature Flag, Token 硬预算)
- **多层 CLAUDE.md 注入**: 全局 (`~/.claude/CLAUDE.md`) + 项目根 + 当前目录 + 条件规则
- CLAUDE.md **作为用户消息附件注入**, 不进 System Prompt (用 `<claude_md_instructions>` 标签)

**三方映射**:

| 能力 | codex | claw-code | ironclaw | Route B |
|------|-------|-----------|---------|---------|
| 基础压缩 (/compact) | ✅ `core/templates/compact/*.md` + `context_manager/` | ✅ `x_claw_agent::compaction` 主实现 | ⚠️ `agent/compaction.rs` shim | ✅ 已归 `dasclaw_context_mgr` W4 |
| 压缩阈值 75-92% 声明 | ⚠️ 未确认 | ⚠️ 未确认 | ❌ | 🟡 **P1 缺口**, 写入 `dasclaw_context_mgr` 配置 |
| **HISTORY_SNIP 选择性剪切** | ❌ | ❌ | ❌ | 🟡 **P1 缺口**, 新算法实现 |
| **CACHED_MICROCOMPACT** | ❌ | ❌ | ❌ | 🟡 **P1 缺口**, 与 `dasclaw_llm_cache` 联动 |
| **TOKEN_BUDGET** | ⚠️ 有 token 统计无硬预算 | ⚠️ | ❌ | 🟡 **P1 缺口** |
| **多层 CLAUDE.md 注入** | ❓ 需验证 | ❓ 需验证 | ❓ | 🟡 **P1 缺口** (需 Round 18 验证) |
| CLAUDE.md 作为消息附件 (非 System Prompt) | ❓ | ❓ | ❓ | 🟡 **P1 缺口** |

**Route B 决策**: 扩充 `dasclaw_context_mgr` 职责, 新增 3 个子模块:
- `context_mgr::snip_compaction` — HISTORY_SNIP 实现
- `context_mgr::micro_compact` — CACHED_MICROCOMPACT 实现
- `context_mgr::token_budget` — TOKEN_BUDGET 实现
- `context_mgr::claude_md_loader` — 多层 claude.md 加载 + 条件规则 + 附件注入

**预计工作量**: 800-1200 LOC (纯策略代码)。

---

### Chapter 06 — Prompt Cache

**Route B 覆盖**: ✅ [11 doc §12.1 第 2 行](./11-target-architecture-route-b.md#12-核心四项能力归属细化-round-12-审计) 已规划 `dasclaw_llm_cache` = claw-code 引擎 735 + ironclaw 监控 175。

**结论**: 覆盖, 无缺口。但需与 Chapter 05 `CACHED_MICROCOMPACT` 联动, 见上条。

---

### Chapter 07 — 多 Agent 协作

**Claude Code 能力**:
- **AgentTool** — 单子 Agent (独立上下文 + depth=1 + 摘要返父)
- **TeamCreateTool** — 团队 Agent (并行 worker + SendMessageTool 通信)
- **Coordinator** (Feature Flag `COORDINATOR_MODE`, 未公开) — Agent 协作层协调器 (Planner/Worker/Reviewer 分工)
- **forkSubagent** (实验性) — 子 Agent fork 父部分上下文

**三方映射**:

| 能力 | codex | claw-code | ironclaw | Route B |
|------|-------|-----------|---------|---------|
| AgentTool | ✅ `multi_agents v1/v2` + `codex_delegate.rs` | ❌ | ✅ `tools/builtin/sub_agent.rs` | ✅ 覆盖 |
| **forkSubagent** | ✅ **`SpawnAgentForkMode::{FullHistory, LastNTurns(usize)}`** (`agent/control.rs:46`) | ❌ | ❌ | ✅ **已在 codex, port 即可** (Round 15/16 发现) |
| **TeamCreateTool** (原子团队创建 + 通信拓扑) | ⚠️ 间接 (循环 SpawnAgent + mailbox) | ❌ | ❌ | 🟢 **P2 缺口**, 需要时新 crate `dasclaw_agent_team` ~600 LOC |
| **Agent Coordinator** (Planner/Worker/Reviewer 协作协议) | ❌ | ❌ | ❌ (orchestrator 是容器层不是 Agent 层) | 🟢 **P3 缺口** (Claude Code 自身都 Feature Flag 藏着) |

**术语消歧** (上轮遗留):
- Claude Code Coordinator = **Agent 协作层**
- ironclaw orchestrator = **容器调度层** (像 Kubernetes)
- 两者层次不同, 不能混谈

**Route B 决策**: 当前不补 TeamCreateTool / Coordinator, 已有 AgentTool + SpawnAgentForkMode 足够常见场景。后续有真实 B 端并行需求再建 `dasclaw_agent_team`。

---

### Chapter 08 — MCP 集成

**Claude Code 能力**: MCP Server 注册 + 工具声明 + 权限隔离。

**Route B 覆盖**: ⚠️ ironclaw `tools/mcp/` 有部分, codex `core/tools/` 可能有。需 Round 18 用 `semantic_search` 验证细节 — 包含 stdio / SSE / WebSocket 三种传输覆盖度。

**结论**: 基础覆盖, 细节待核。

---

### Chapter 09 — 启动性能优化

**Claude Code 能力**:
- `profileCheckpoint` — 入口性能追踪
- 并行预取 — 利用 JS import 求值顺序, MDM/Keychain/GrowthBook 与 135ms 模块加载并行
- 懒加载破循环依赖
- **Bun `feature()` 构建时死代码消除**

**三方映射**:

| 能力 | codex | claw-code | ironclaw | Route B |
|------|-------|-----------|---------|---------|
| 启动追踪 (profileCheckpoint 等价) | ❓ | ❓ | ❓ | 🟢 **P2 缺口**, Rust 用 `tracing` span + 启动 phase 标记 |
| 并行预取 | ⚠️ Rust 无 JS import 副作用陷阱, 需显式 `tokio::spawn` | ❓ | ❓ | 🟢 **P2 缺口**, main.rs 入口补并行 task |
| 懒加载 | ⚠️ Rust 用 `once_cell::Lazy` / `OnceLock` | 部分 | 部分 | 🟢 **P2 约定**, 重模块统一用 `OnceLock` |
| **构建时死代码消除** | ⚠️ Rust 用 `#[cfg(feature = "...")]` 原生支持 | ✅ Cargo feature | ✅ Cargo feature | ✅ **Rust 天然覆盖** (比 bun:bundle 更完善) |

**Route B 决策**: 启动优化属于**工程细化**, 不新增 crate, 在现有各 crate 里按需应用:
- 在 `desktop-client/src/main.rs` 和 `admin-backend/src/main.rs` 补并行 `tokio::spawn` 预取
- 重模块 (config/token cache/feature flags) 用 `OnceLock` 懒加载
- Rust 的 `#[cfg(feature)]` 已等价 bun:bundle feature(), 不需额外工作

**预计工作量**: 200-400 LOC, 散落在各 crate。

---

### Chapter 10 — Feature Flag 体系

**Claude Code 能力**:
- **构建时 Feature Flag** — `bun:bundle feature()`, 未启用代码被物理删除
- **运行时 GrowthBook** — A/B 测试 + 灰度发布
- **20+ 隐藏 Feature Flag**:

| Flag | 含义 | 状态 |
|------|------|------|
| `KAIROS` | 助理模式 (Sleep/Push/Brief/SubscribePR 等工具栈) | 未公开 |
| `PROACTIVE` | 主动模式 — AI 自动执行任务 | 未公开 |
| `COORDINATOR_MODE` | 多 Agent 协调器 | 未公开 |
| `AGENT_TRIGGERS` | 定时任务触发器 | 未公开 |
| `AGENT_TRIGGERS_REMOTE` | 远程触发 | 未公开 |
| `MONITOR_TOOL` | 监控工具 | 未公开 |
| `BRIDGE_MODE` | IDE 集成桥接 | **已启用** |
| `DAEMON` | 后台守护进程 | 未公开 |
| `HISTORY_SNIP` | 历史剪切压缩 | 实验中 |
| `CACHED_MICROCOMPACT` | 缓存微压缩 | 实验中 |
| `TOKEN_BUDGET` | Token 预算控制 | 实验中 |
| `EXPERIMENTAL_SKILL_SEARCH` | 技能搜索 | 实验中 |
| `UDS_INBOX` | `/peers` 进程间通信 | 未公开 |
| `VOICE_MODE` | 语音输入 | 未公开 |
| `BUDDY` | 电子宠物伴侣 | 隐藏彩蛋 |
| `ULTRAPLAN` | `/ultraplan` 超级规划 | 未公开 |

**三方映射**:

| 能力 | codex | claw-code | ironclaw | Route B |
|------|-------|-----------|---------|---------|
| 构建时 Flag | ✅ `#[cfg(feature)]` 原生 | ✅ | ✅ | ✅ 已覆盖 |
| **运行时 Flag (A/B + 灰度)** | ❌ | ❌ | ❌ | 🟡 **P1 缺口**, 新 crate `dasclaw_feature_flags` 集成 OpenFeature 或自研 |
| 远程配置 (GrowthBook) | ❌ | ❌ | ❌ | 🟡 **P1 缺口**, 后端集成 Unleash / Flagsmith / 自建 |

**Route B 决策**:
- 新 crate `dasclaw_feature_flags`, 基于 [OpenFeature Rust SDK](https://github.com/open-feature/rust-sdk)
- 提供统一接口: `feature_flag!("KAIROS")` 宏 + 运行时查询
- 后端: admin-backend 集成 Flagsmith 自建实例 (开源 + B 端友好)

**预计工作量**: 400-600 LOC (crate 壳 + provider 适配)。

**具体 Feature 的取舍**:
- 🔴 `HISTORY_SNIP` / `CACHED_MICROCOMPACT` / `TOKEN_BUDGET` — 见 Chapter 05, P1 补
- 🟢 `KAIROS` 系 — P2 暂不, 需产品决策
- 🟢 `COORDINATOR_MODE` — P3 暂不 (见 Chapter 07)
- 🟢 `DAEMON` — P2, desktop-client 后期可能需要
- 🟢 `AGENT_TRIGGERS_REMOTE` — P2, 扩展 ironclaw routines
- 🟢 `MONITOR_TOOL` — P2, KAIROS 配套
- 🟢 `EXPERIMENTAL_SKILL_SEARCH` — P1, `dasclaw_skills` 扩展
- 🟢 `BRIDGE_MODE` — P2, claw-code 可能已有部分, 需验证
- ⚪ `VOICE_MODE` / `BUDDY` / `ULTRAPLAN` — 产品特性, 暂不

---

### Chapter 11 — 安全机制

**Claude Code 能力**:
- Bash AST 分析 7000+ 行 (见 Chapter 03)
- Prompt Injection 防御 (见 Chapter 01 `<system-reminder>` + CYBER_RISK_INSTRUCTION)
- 5 种权限模式 (见 Chapter 04)
- 文件系统沙箱 + 网络访问控制 (见 [13 doc §5](./13-security-capability-inventory.md))
- **CVE 跟踪** (CVE-2026-21852 / CVE-2026-33068 等)

**三方映射**:

| 能力 | codex | claw-code | ironclaw | Route B |
|------|-------|-----------|---------|---------|
| OS 沙箱 (landlock/Seatbelt) | ✅ | ❌ | ❌ | ✅ 已归 codex, 见 13 doc |
| Bash AST 分析 | ❌ | ❌ | ❌ | 🔴 **P0 缺口**, `dasclaw_bash_guard` (见 Chapter 03) |
| Prompt Injection 标签 | ❓ | ❓ | ❓ | 🔴 **P0 缺口** (见 Chapter 01) |
| CYBER_RISK_INSTRUCTION | ❓ | ❓ | ❓ | 🔴 **P0 缺口** (见 Chapter 01) |
| **CVE 跟踪流程** | ❌ | ❌ | ❌ | 🔴 **P0 缺口**, 建立 `docs/security/cve-tracking.md` + CI 扫描 |

---

## 2. 缺口汇总 (按优先级)

### 2.1 🔴 P0 必补 (安全相关, 生产阻塞级)

| # | 能力 | 所属层 | 新增 crate / 模块 | 工作量 |
|---|------|-------|------------------|--------|
| 1 | Bash AST 安全分析 7000+ 行 | L5 safety | `dasclaw_bash_guard` | 3000-5000 LOC |
| 2 | sed/awk 修改标志识别 | L5 safety | `dasclaw_bash_guard::sed_awk` | 500 LOC |
| 3 | `<system-reminder>` Prompt Injection 标签 | L2 context | `dasclaw_context_mgr::injection_guard` | 200 LOC |
| 4 | CYBER_RISK_INSTRUCTION 安全约束文本 | L2 context | `dasclaw_context_mgr::safety_instructions` | 100 LOC |
| 5 | CVE 跟踪流程 + CI 扫描 | 工程 | `docs/security/cve-tracking.md` + `scripts/ci/cve-scan.sh` | 200 LOC + 文档 |

**P0 总工作量**: ~4000-6000 LOC + 文档。

### 2.2 🟡 P1 应补 (工程基础设施)

| # | 能力 | 所属层 | 新增 crate / 模块 | 工作量 |
|---|------|-------|------------------|--------|
| 6 | 运行时 Feature Flag (OpenFeature / Flagsmith) | 横切 | `dasclaw_feature_flags` | 400-600 LOC |
| 7 | HISTORY_SNIP 选择性剪切压缩 | L2 context | `dasclaw_context_mgr::snip_compaction` | 400 LOC |
| 8 | CACHED_MICROCOMPACT 缓存微压缩 | L2 context | `dasclaw_context_mgr::micro_compact` | 300 LOC |
| 9 | TOKEN_BUDGET 硬预算 | L2 context | `dasclaw_context_mgr::token_budget` | 200 LOC |
| 10 | 多层 CLAUDE.md 加载 + 条件规则 + 附件注入 | L2 context | `dasclaw_context_mgr::claude_md_loader` | 300-500 LOC |
| 11 | 压缩阈值 75-92% 配置化 | L2 context | `dasclaw_context_mgr::threshold_config` | 50 LOC |
| 12 | EXPERIMENTAL_SKILL_SEARCH | L4 tools | `dasclaw_skills::search` | 400 LOC |

**P1 总工作量**: ~2000-2500 LOC。

### 2.3 🟢 P2 可选 (产品能力 + 性能优化)

| # | 能力 | 备注 |
|---|------|------|
| 13 | 启动性能优化 (tracing spans + OnceLock + 并行 spawn) | 散落各 crate, ~300 LOC |
| 14 | DAEMON 后台守护进程 | desktop-client 需要时再做 |
| 15 | AGENT_TRIGGERS_REMOTE 远程触发 | ironclaw routines 扩展 |
| 16 | MONITOR_TOOL 代码库监控 | KAIROS 配套 |
| 17 | BRIDGE_MODE IDE 集成 | claw-code 可能已有, 需 Round 18 验证 |
| 18 | TeamCreateTool 原子并行团队 | ~600 LOC, 有真实并行需求再做 |
| 19 | UDS_INBOX (`/peers` 进程间通信) | 小众特性 |

### 2.4 ⚪ P3 / 暂不考虑

- KAIROS 主动模式 (产品决策)
- Agent Coordinator Feature Flag 模式 (Claude Code 自己都藏)
- `/ultraplan` `/torch` 等单命令特性
- `VOICE_MODE` / `BUDDY` (非 B 端刚需)

---

## 3. Wave 路线图补丁 (对 [11 doc §8](./11-target-architecture-route-b.md#8-wave-路线图-路线-b-完整版) 的修订)

建议在现有 Wave 结构中插入以下新 Wave:

| Wave | 原计划 | 补丁 |
|------|-------|------|
| W0 (新) | — | **安全 P0 补齐**: `dasclaw_bash_guard` + Prompt Injection 标签 + CYBER_RISK_INSTRUCTION + CVE 流程 |
| W4 | context_mgr + llm_cache | **追加**: `snip_compaction` + `micro_compact` + `token_budget` + `claude_md_loader` + `threshold_config` (P1 能力融入) |
| W5 (新) | — | **Feature Flag 基础设施**: `dasclaw_feature_flags` 独立 crate, 为后续 KAIROS/BRIDGE_MODE 等 Flag 提供底座 |
| W9+ | — | **P2 能力按需**: TeamCreateTool / DAEMON / 启动优化 / SKILL_SEARCH 等, 滚动排期 |

---

## 4. 后续动作

### 4.1 Round 18 待验证项 (用 `semantic_search`, 不用 `rg`)

1. codex 是否有 `CYBER_RISK_INSTRUCTION` 等价? (搜 "security warning" / "prompt injection" / "anthropic safety")
2. codex 是否有 `<system-reminder>` 等价标签? (搜 "system reminder" / "tag system message" / "distinguish user message")
3. codex/claw-code 压缩阈值具体配置? (搜 "compaction threshold" / "token limit")
4. claude.md 多层加载是否已实现? (搜 "claude md loader" / "global project directory md")
5. BRIDGE_MODE 在 claw-code 是否有? (搜 "ide bridge" / "editor integration")
6. MCP 传输覆盖度 (stdio / SSE / WebSocket)? (搜 "mcp transport")

### 4.2 新 crate 命名检查

本 doc 提出的新 crate, 请对照 [11 doc §2 依赖关系图](./11-target-architecture-route-b.md#2-crate-依赖关系图) 插入:

- `dasclaw_bash_guard` → 依赖 `dasclaw_safety_core`, 被 `dasclaw_tools_kernel::bash` 调用
- `dasclaw_feature_flags` → 顶层横切, 无依赖, 被几乎所有 crate 消费
- `dasclaw_agent_team` (P2) → 依赖 `dasclaw_agent_spawn`

### 4.3 工具使用规范落地

本 doc §0 的"三级工具使用规范"应进 `AGENTS.md` 或 `.github/instructions/` 作为强制流程, 避免未来再出现字面量搜索导致的误判。

---

## 5. 变更历史

- **Round 17 (本 doc 创建)**: 系统扫描 decode-claude-code-main/ 12 章, 建立全量能力对账, 识别 19 项缺口分 P0-P3 四级。
