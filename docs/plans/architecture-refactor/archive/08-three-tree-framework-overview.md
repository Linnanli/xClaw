# 三仓框架画像（Phase 4 前置调研 · 阶段 1）

> **目的**：在进入模块级对比前，先用工具（tokei / cargo-modules / aider repo-map）把 `claw-code` / `codex-cli-main` / `desktop-client/ironclaw` 三个代码库的整体框架画清楚，避免凭印象做决策。
>
> **产出**：本文件 = 阶段 1 总结。原始 repo-map 位于 `/tmp/xclaw-analysis/repomap-{claw,codex,ironclaw}.txt`。
>
> **下一步**：阶段 2 按主题聚焦（Agent / Tool / Skill / 权限沙箱 / Hook）。

---

## 1. 规模画像（tokei Rust-only）

| 仓库 | Rust 文件数 | Rust LOC | crate/模块数 | 主体定位 |
|---|---:|---:|---:|---|
| **claw-code/rust** | 80 | 69,638 | 9 crate | Claude Code CLI 的 Rust 克隆，含 `compat-harness` / `mock-anthropic-service` 做 API 级 parity |
| **codex-cli-main/codex-rs** | ~1,600 | 644,578 | **95 crate** | OpenAI Codex CLI 生产级实现，Apache-2.0，含沙箱 / TUI / app-server / realtime / plugin / skills 全栈 |
| **desktop-client/ironclaw** | 507 | 232,729 | 主 crate + 5 channels + 10 tools + 2 共享 crate | OpenClaw Rust 重实现，含 20+ 通道 / Extension 统一抽象 / DLP |

**关键观察**：
- codex 体量约是 claw-code 的 **9 倍**、ironclaw 的 **2.8 倍**
- claw-code 虽然最小，但**结构最整洁**（9 crate 职责清晰）
- ironclaw 是"单 crate + 子项目"结构，主 crate `src/` 就有 30 个顶层模块

---

## 2. crate / 模块拓扑

### 2.1 claw-code（9 个 crate，职责单一）

| crate | LOC | 职责 |
|---|---:|---|
| `runtime` | 30,647 | Agentic 循环、session、task_registry、validate_packet |
| `rusty-claude-cli` | 16,152 | CLI 主入口、TUI |
| `tools` | 10,414 | 40 个工具实现（bash/file_ops/Agent/WebSearch 等） |
| `api` | 9,662 | Anthropic / OpenAI / Cohere 等多 provider 客户端 |
| `commands` | 5,634 | 斜杠命令（/skills, /agents, /subagent, /plugin 等） |
| `plugins` | 4,294 | Plugin 加载 |
| `mock-anthropic-service` | 1,157 | Parity harness 用的本地 mock |
| `telemetry` | 526 | 轻量埋点 |
| `compat-harness` | 363 | 对拍原版 Claude Code CLI 行为 |

**特点**：**Parity 工程纪律极强** —— 有专门的 `mock-anthropic-service` + `compat-harness` 确保跟 Claude Code 行为一致。

### 2.2 codex-rs（95 crate，三层金字塔）

按 LOC 规模排序的 Top 30（完整 95 个见 `Cargo.toml`）：

**🏔️ 重量级（>50k LOC）**
- `core` (217,938) — Agentic 核心
- `tui` (142,167) — Terminal UI
- `app-server` (78,434) — 长驻服务

**🏔️ 中量级（10k-20k）**
- `app-server-protocol` (19,947) / `protocol` (15,712) — 协议层
- `exec-server` (13,615) / `state` (12,768) / `windows-sandbox-rs` (12,545)
- `tools` (10,804) / `utils` (10,440) / `codex-api` (10,385)

**🏔️ 专业化（5k-10k）**
- `rollout-trace` / `rollout` / `config` / `network-proxy` / `login`
- `exec` / `rmcp-client` / `cli` / `hooks`
- `core-plugins` / `core-skills`
- `linux-sandbox` / `sandboxing` / `shell-command`
- `otel` / `analytics` / `cloud-tasks` / `git-utils` / `codex-mcp`

**🏔️ 小型工具（<5k，60+ 个）**
- `features`（Feature flag 中枢）/ `apply-patch` / `skills` / `plugin`
- `execpolicy` / `process-hardening` / `device-key` / `keyring-store`
- `realtime-webrtc` / `lmstudio` / `ollama` / `model-provider`
- `utils/*`（25 个子 crate：absolute-path / pty / readiness / approval-presets / sandbox-summary ...）

**特点**：**极度模块化**，一个功能一个 crate。Feature flag（60+）+ 小 crate 组合，**可独立发布、独立编译、独立测试**。

### 2.3 ironclaw（单 crate + 30 个模块 + 17 个子 crate）

**主 crate `src/` 顶层 30 个模块（按 LOC 排序）**：

| 模块 | LOC | 职责 |
|---|---:|---|
| `tools/` | 50,928 | 工具注册、MCP、内置工具实现 |
| `channels/` | 40,379 | 20+ 聊天通道（Telegram/Slack/Discord/Feishu/...） |
| `llm/` | 28,298 | Provider 抽象、流式推理 |
| `extensions/` | 11,786 | **Channel+Tool+MCP 三合一**抽象 |
| `cli/` | 10,889 | CLI 入口 |
| `agent/` | 9,579 | Agentic 循环 |
| `routines/` | 8,662 | Phase 3-H'' 提升后的 routines 子系统 |
| `db/` | 8,298 | 数据层 |
| `config/` | 7,274 | 配置 |
| `workspace/` | 6,691 | 工作区管理 |
| `setup/` | 6,361 | 首次安装向导 |
| `worker/` | 5,342 | 异步 worker |
| `skills/` | 3,662 | **SKILL.md Trust 信任模型**（Trusted/Installed + attenuate_tools） |
| `sandbox/` | 3,611 | 沙箱 |
| `history/` | 3,268 | 会话历史 |
| `registry/` | 3,197 | 注册表 |
| `orchestrator/` | 3,188 | 编排 |
| `context/` | 2,824 | 上下文管理 |
| `secrets/` | 2,542 | 密钥 |
| `hooks/` | 2,490 | Hook 系统 |
| 其他 10 个 | ~20k | safety / testing / observability / tunnel / webhooks / import / evaluation / estimation / pairing / document_extraction |

**17 个子 crate**：
- `channels-src/`：discord / feishu / slack / telegram / whatsapp（5 个）
- `tools-src/`：github / gmail / google-{docs,calendar,drive,sheets,slides} / llm-context / slack / telegram / web-search（10 个）
- `crates/`：ironclaw_common / ironclaw_safety（2 个）

**特点**：**单 crate + 目录模块化**，扩展能力（channels/tools-src）走 WASM 子 crate。

---

## 3. 三仓架构风格对比

| 维度 | claw-code | codex | ironclaw |
|---|---|---|---|
| 组织形态 | 9 crate workspace | 95 crate workspace | 1 主 crate（30 模块）+ 17 子 crate |
| 拆分粒度 | 按**功能域**（api/tools/commands/runtime/cli） | 按**单一职责**（一个能力一个 crate） | 按**目录模块**（扩展能力独立子 crate） |
| 可独立发布 | ⚠️ 部分 | ✅ 95 个都能独立发布 | ❌ 主 crate 一体 |
| 编译耗时估计 | 短 | **极长**（95 crate） | 长（单 crate 有 190k LOC 实质代码） |
| 学习曲线 | 低（目录清晰） | 高（crate 多需索引） | 中（主 crate 内部依赖多） |
| Feature flag 治理 | 基础 | ✅ 60+ Feature 中枢 | 基础 |
| 协议层独立 | ❌ 无 | ✅ `protocol` / `app-server-protocol` / `codex-api` | ⚠️ 混在各模块 |
| 沙箱层独立 | ⚠️ `runtime` 内 | ✅ `linux-sandbox` + `windows-sandbox-rs` + `sandboxing` 三个 crate | ⚠️ `src/sandbox/` 内 |
| Hook 系统 | 无独立 | ✅ `hooks` crate + `core/src/hooks/` | ✅ `src/hooks/` |

---

## 4. 三仓能力并集 vs ironclaw 当前覆盖

基于 repo-map + crate 列表，把**能力维度**拆出来看 ironclaw 相对 claw-code + codex 的覆盖情况：

| 能力域 | claw-code | codex | ironclaw | 结论 |
|---|---|---|---|---|
| **Agentic 循环** | ✅ `runtime` | ✅ `core` | ✅ `agent/` | 三方都有 |
| **Tool 系统** | ✅ `tools` 40 tool | ✅ `tools` + `core/tools/handlers` | ✅ `tools/` 50k LOC | ironclaw 最大 |
| **子 agent 并行** | ✅ `runtime/task_registry` | ✅ `agent_job` + `spawn_agent` + WebSocket | ⚠️ **UI 占位、后端未实现** | **ironclaw 缺口** |
| **Plan Mode** | ✅ `/plan` + Plan agent | ✅ `core/config/agent_roles` | ❌ 无 | **ironclaw 缺口** |
| **Session Fork** | ✅ `/session-fork` + TUI | ✅ `rollout` + `state` | ❌ 无 | **ironclaw 缺口** |
| **Skill 系统** | ✅ `commands/skills` | ✅ `skills` + `core-skills`（thin） | ✅ **SKILL.md Trust 模型最强** | ironclaw 最强 |
| **Plugin 系统** | ✅ `plugins` crate | ✅ `plugin` + `core-plugins` + `utils/plugins` | ⚠️ 用 `extensions/` 覆盖 | **抽象不同，但 ironclaw 更全** |
| **Extension（Channel+Tool+MCP 统一）** | ❌ 无 | ❌ 无 | ✅ 独家 | **ironclaw 独家优势** |
| **MCP 集成** | ⚠️ via commands | ✅ `codex-mcp` + `mcp-server` + `rmcp-client` | ✅ ExtensionKind::McpServer | 三方都有，codex 最深 |
| **通道（Telegram/Slack/...）** | ❌ 无 | ❌ 无 | ✅ **20+ 通道** | **ironclaw 独家优势** |
| **沙箱** | ⚠️ 轻 | ✅ **linux-sandbox + windows-sandbox 原生** | ⚠️ `src/sandbox/` 需强化 | **ironclaw 缺口** |
| **权限策略（execpolicy）** | ⚠️ 工具级 | ✅ `execpolicy` + `execpolicy-legacy` 独立 | ⚠️ 分散 | **ironclaw 缺口** |
| **Hook 系统** | ⚠️ 基础 | ✅ `hooks` crate + 声明式 | ✅ `src/hooks/` | 需对比深度 |
| **Apply Patch** | ⚠️ 通过 Edit tool | ✅ `apply-patch` 独立 crate | ⚠️ 无独立抽象 | **ironclaw 缺口** |
| **Git 集成** | ✅ tools 内 | ✅ `git-utils` 独立 | ⚠️ 分散 | **ironclaw 缺口** |
| **LSP 查询** | ❌ 无 | ⚠️ 间接 | ⚠️ 占位 | **三方都弱** |
| **apply-patch diff 校验** | ✅ tools/edit | ✅ `apply-patch` | ⚠️ 基础 | **ironclaw 缺口** |
| **协议层** | ⚠️ `api` 内 | ✅ `protocol` + `codex-api` + `app-server-protocol` | ⚠️ 分散 | **ironclaw 治理缺失** |
| **云任务/分布式** | ❌ 无 | ✅ `cloud-tasks*` + `cloud-requirements` | ❌ 无 | 非关键 |
| **实时协作** | ❌ 无 | ✅ `realtime-webrtc` + `collaboration-mode-templates` | ❌ 无 | 非关键 |
| **TUI** | ✅ 基础 | ✅ **`tui` 142k LOC，生产级** | ⚠️ 前端用 Tauri+React，非 TUI | ironclaw 走 GUI 路线 |
| **多 channel 分布（app-server）** | ❌ 无 | ✅ `app-server` 78k LOC | ⚠️ 部分 | **ironclaw 缺口** |
| **OTel 可观测** | ⚠️ telemetry crate | ✅ `otel` + `analytics` | ⚠️ `observability/` | **ironclaw 缺口** |
| **Feature flag 中枢** | ❌ 无 | ✅ `features` crate（60+） | ❌ 无 | **ironclaw 缺口** |
| **Model provider 生态** | ✅ `api` 多家 | ✅ `model-provider` + `lmstudio` + `ollama` 等 | ✅ `llm/` 28k LOC | ironclaw 次于 codex |
| **Parity 验证基础设施** | ✅ `mock-anthropic-service` + `compat-harness` | ⚠️ 内部测试 | ❌ 无 | **claw-code 独家** |

---

## 5. Phase 4 前置调研的关键修正

对照 `06-phase4-claw-code-capability-port.md` 原计划，阶段 1 产出修正以下：

### 5.1 原计划的**不准确假设**

| 原计划陈述 | 实际情况（阶段 1 证据） | 修正方向 |
|---|---|---|
| "claw-code 只是参考，只有 `bash_validation.rs` 被真实移植" | ✅ 确认 —— 但**不代表借鉴不够**，claw-code 的 `runtime` crate 有 30k LOC agentic 循环 + task_registry，**值得整体结构学习** | 将 `runtime/task_registry.rs` 模式明确列入 P0 |
| "ironclaw 的 agentic_loop 是 OpenClaw 血统" | ✅ 确认 —— 且 `src/agent/` 仅 9,579 LOC，**相对单薄** | agent 层是**次重点**，要补足子 agent / Plan Mode / Session Fork |
| "codex 只作为架构参考" | ⚠️ 不准确 —— codex 的 `features`、`apply-patch`、`execpolicy`、`hooks`、`git-utils` 等**小 crate 可直接整体借鉴甚至移植**（Apache-2.0） | 列入**允许整 crate 移植**的候选清单 |
| "ironclaw skills 比 codex 更强" | ✅ 确认 —— ironclaw `skills/` 有 Trust 模型 + attenuate_tools，codex `skills/` 只是 thin installer | skills 层**保持 ironclaw 现状** |
| "多 agent 协作只有 codex 有" | ❌ 错 —— claw-code `runtime/task_registry.rs` + `/subagent` / `/team` 命令也完整 | Agent 层改为**"从 claw-code 抄实现、从 codex 抄架构"** |

### 5.2 原计划**遗漏的能力**（阶段 1 发现）

1. **Feature flag 中枢**（codex `features` crate，60+ flag）— ironclaw 没有，治理必需
2. **Apply-patch 独立抽象**（codex `apply-patch` crate）— diff 安全应用，政企审计友好
3. **Execpolicy / 沙箱层化**（codex `execpolicy` + `linux-sandbox` + `windows-sandbox-rs` + `sandboxing` 四个 crate）— ironclaw 目前 `src/sandbox/` 3,611 LOC，**不够**
4. **Parity 验证基础设施**（claw-code `mock-anthropic-service` + `compat-harness`）— 长期回归验证利器
5. **协议层独立**（codex `protocol` + `codex-api` + `app-server-protocol`）— ironclaw 协议散布在各模块，**重构良机**
6. **Git utils 独立**（codex `git-utils`）— 与 Plan Mode / Session Fork 相关

---

## 6. 阶段 2 主题聚焦优先级（重新排序）

基于阶段 1 的证据和 ironclaw 缺口的**严重程度 × 政企价值**重新排序：

| 优先级 | 主题 | 理由 | 预期产出 |
|---|---|---|---|
| **P0.1** | **Agent / 多 agent 编排** | ironclaw 仅 UI 占位、后端未实现；claw-code 和 codex 都有成熟方案；政企审批链/工单链的基座 | `08-a-agent-comparison.md` |
| **P0.2** | **Tool 系统 + 注册机制** | ironclaw `tools/` 50k LOC 已有但缺 codex 式 Feature flag 门控；ToolProfile 切换架构的关键 | `08-b-tool-comparison.md` |
| **P0.3** | **权限 / 沙箱 / execpolicy** | ironclaw 3.6k LOC 太轻；政企强约束；codex 4 个 crate 成熟方案可借鉴 | `08-c-sandbox-comparison.md` |
| **P1.1** | **Hook 系统** | ironclaw 2.5k LOC 需对比深度；政企审批/回调核心 | `08-d-hook-comparison.md` |
| **P1.2** | **Apply-patch / Edit 流程** | 政企代码审计刚需；codex 有独立 crate | `08-e-edit-comparison.md` |
| **P1.3** | **Skill 系统** | ironclaw 已领先，做深度确认 + 借鉴 codex 的 embed/MCP 依赖 | `08-f-skill-comparison.md` |
| **P2** | Feature flag 中枢 / Parity 基础设施 / Plan Mode / Session Fork | 工程治理类 | 合并一份 |
| **P3** | 协议层 / Git-utils / 可观测 | 长期架构 | 合并一份 |

---

## 7. 工具产出清单

| 产物 | 路径 | 大小 |
|---|---|---|
| claw-code repo-map | `/tmp/xclaw-analysis/repomap-claw.txt` | 2,118 行 / 59 KB |
| codex repo-map | `/tmp/xclaw-analysis/repomap-codex.txt` | 4,878 行 / 128 KB |
| ironclaw repo-map | `/tmp/xclaw-analysis/repomap-ironclaw.txt` | 3,605 行 / 98 KB |

这三个文件**阶段 2 各主题聚焦时按 include pattern 再过滤 Repomix 输出使用**，不重复跑 aider。

---

## 8. 下一步决策点

请用户确认以下三项：

1. **主题聚焦顺序**：是否接受"P0.1 Agent → P0.2 Tool → P0.3 Sandbox → P1 Hook/Patch/Skill → P2/P3 合并"？
2. **codex 整 crate 移植授权**：是否允许在 Phase 4 中直接将 codex 的 `features` / `apply-patch` / `git-utils` 等**小 crate 整体移植**进 ironclaw（Apache-2.0 兼容 MIT）？还是只**参考 + 重写**？
3. **Phase 4 修订时机**：阶段 2 每个主题做完就增量更新 `06-phase4-...md`？还是阶段 2 全做完再一次性改写为 `06-phase4-revised.md`？
