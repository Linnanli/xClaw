# 32 — 分阶段执行计划

> **v2.5 (2026-05-02)** · 配套 [ADR-118](adr-118-claw-code-readonly-and-self-impl.md)：W6 新增任务组 C「LLM provider 中立迁出 + 删 claw-code-api path-dep」。`crates/dasclaw_llm_provider` 新建 + `desktop-client/ironclaw/src/llm/claw_code_provider.rs` 切换 + 删 `Cargo.toml:149` path-dep。claw-code 子仓**保持只读参考库**（ADR-117 D5/D7 子仓内删除已撤回）。W6 工作量 +0.5 周（5 -> 5.5 周）。

> **v2.4 (2026-04-26)** · 新增「通用 Wave 完成准则」一节：每个 Wave 任意大功能块 commit 之前，必须依次跑 `code-simplifier`（清理）+ `code-review-expert`（严肃 review，P0/P1 阻塞 commit）。三个 review skill 已对比评估，code-review-expert 为唯一首选（SOLID + 语言特化 + 架构异味）。

> **v2.3 (2026-04-26)** · W1 任务 1 订正：不删 fork、不升级 0.26。git log 实测 fork 有 42 fork-only commit（Phase 2/3 dasclaw 接线核心成果），W1-W6 保留作为私货来源，W6+ 才删除。与 31 v2.3 + 38 §137 对齐。

> **v2.2 (2026-04-25)** · 配套 [31-target-architecture.md](31-target-architecture.md) v2.2 + [30-architecture-truth.md](30-architecture-truth.md) v2 + [35-codex-capability-inventory.md](35-codex-capability-inventory.md) v0.3。
> 共 9 个 Wave，每个 Wave 2-5 周。总计 ~8-10 个月（v2.2 比 v2.1 +1 月，吸收 ironclaw bridge_lite + workspace 去多租户化 + codex 88 commit 增量跟进）。
>
> **v2.1 → v2.2 变更**：
> 1. **W6 从 4 周扩到 5 周**：吸收 ironclaw 后端基底（dasclaw_workspace 7-9k LOC + dasclaw_bridge_lite 5-7k LOC）+ codex 88 commit P0 增量（goal 系统 / ThreadStore trait / permissions profiles / rollout-trace / Unix socket 评估）。
> 2. **dasclaw_core 工作量精算**：从 "v2 全套 30k LOC" 修正为 **14-16k LOC**（v2 trait 抽象只占 410 LOC + types 3,109 LOC，砍至仅 chat+job 两类 thread type 的 10-12k 实现 = ADR-104 v2.2 补充）。
> 3. **dasclaw_bridge_lite ≈ 5-7k LOC**（vs ironclaw bridge 全套 25,369 LOC，ADR-110 新增）。
> 4. **dasclaw_workspace ≈ 7-9k LOC**（vs ironclaw workspace 全套 12,857 LOC，去多租户化 -3k LOC）。
> 5. **goal 系统五件套**纳入 W6（codex 88 commit P0★★★★★ 增量）。
>
> **v2.1 (2026-04-25)** 配套 [31-target-architecture.md](31-target-architecture.md) v2 + [30-architecture-truth.md](30-architecture-truth.md) v2。
> v2.0 共 9 个 Wave，每个 Wave 2-4 周。总计 ~7-9 个月（与 27 文档"架构优先"方向一致，比 v1 多 1 月涵盖新增 crate）。
> 每个 Wave 都有：进入条件、产出、验收标准、回滚点。
>
> **v1 → v2 变更**：
> 1. W1 crate 骨架从 12 个加到 **15 个**（+project_docs / +pty / +crash / +net_proxy / +bash_validation）
> 2. W3 加入 **AGENTS.md 加载器**任务（ADR-106）
> 3. W6 明确补齐 LSP IPC / MCP IPC / Approval WebSocket（ADR-107a/108）
> 4. W7 加入 panic hook + crash 收拢（ADR-109） + AuthToken refresh 流（ADR-107b）
> 5. W2 加入 **portable-pty / exec-server port**任务。
>
> **v2.1 (2026-04-25) 二轮修订**— 反映用户二次反馈：
> 1. 全量重命名：除 `crates/x_claw_agent` 保留不动外，所有新 crate / 路径 / ID 一律使用 `dasclaw_*` 前缀。
> 2. 同步 31 文档 ADR-101 从“升级到主仓”调整为“吸收式重构”路径。
> 3. ADR-107 拆分为 107a（Approval WebSocket）+ 107b（AuthToken 客户端 refresh + admin 审计）。
> 4. 以 0.3.x 起步（不是 1.x）。
> 5. 与上游 fork ironclaw v0.26 「实质不同」（fork 御同 PR 只制）：ADR-101 重估后差异是 6,496 LOC（6 个核心文件）+ 8 个 upstream 仅有模块 + 1,642 LOC fork-only，不是 2-3 天合并问题。
>
> **v2.1 (2026-04-25) 三轮修订**— 响应用户三指令：
> 1. W2 从 4 周扩到 5 周：dasclaw_workspace_cap 策略层 + WritableRoot 洞中洞契约测试重质量。
> 2. W3 显式列出 **E 类多层 AGENTS.md/CLAUDE.md 加载 task**（之前只隐含在 dasclaw_project_docs），改用 codex 多层方案而非 claw 单层。
> 3. 增加 [36-claw-code-capability-inventory.md](36-claw-code-capability-inventory.md) 全量能力清单作为 W4/W5 移植依据。
>
> **v2.0 → v2.1 变更**：
> - **命名约定**：新增 crate 全部使用 **`dasclaw_*`** 前缀（用户决策 / 11 文档方案 B）
> - 已存在的 `crates/x_claw_agent` 保留原名（已实现 LoopDelegate Route B），逐步迁移
> - **ADR-107 拆分**为 ADR-107a（Approval WebSocket）+ ADR-107b（AuthToken 续期），两件事独立验收
> - **澄清"双引擎合并"**：实际是 `desktop-client/ironclaw/src/agent/dispatcher.rs`（旧）→ `dasclaw_core`（新，由 x_claw_agent 升级），不是 CodeAct v1↔v2。CodeAct v2 引擎在 ironclaw-main 0.26 上游，桌面端**暂不激活**（无 Python 解释器场景）。
> - **W2 加入 dasclaw_workspace_cap 策略层** — 借鉴 codex SandboxPolicy enum（用户 Q2），仅 desktop 本地不扩 admin（用户 Q1），保留现有 cap-std 强制层。
> - **配套新文档**：[35-codex-capability-inventory.md](35-codex-capability-inventory.md)（codex 全量盘点 + 工作区隔离深度章节）。

---

## Wave 概览

| Wave | 主题 | 耗时 | 关键产出 | 阻塞下一 Wave 的产出 |
|------|------|------|---------|-------------------|
| **W1** | 基线收敛 | 3 周 | 删 fork、合并 runtime、15 crate 骨架就位 | dasclaw_core 雏形 |
| **W2** | Sandbox 三平台 + PTY + Workspace 策略层 | **5 周**（v2.1 +1 周，因 dasclaw_workspace_cap） | dasclaw_sandbox（Linux+macOS+Win）+ dasclaw_pty + dasclaw_workspace_cap | sandbox trait + pty trait + SandboxPolicy enum |
| **W3** | Hooks 引擎 + Apply Patch + 项目文档（含 E 类多层加载） | 3 周 | dasclaw_hooks（schema/registry/engine）+ dasclaw_apply_patch + dasclaw_project_docs（**E 类显式 task**） | hooks trait + patch trait + AGENTS.md/CLAUDE.md 多层加载 |
| **W4** | 治理 6 件套 + bash_validation | 3 周 | dasclaw_governance + dasclaw_bash_validation | governance trait |
| **W5** | MCP 统一 + ExecPolicy | 2 周 | dasclaw_mcp（6 transport）+ dasclaw_execpolicy | — |
| **W6** | 客户端外壳整合 + **后端基底吸收**（v2.2） | **5 周**（v2.2 +1 周） | dasclaw_workspace 7-9k + dasclaw_bridge_lite 5-7k + desktop-client 全部 IPC 切到新 runtime + LSP/MCP/Goal IPC 补齐 + Approval WebSocket + codex 88 commit P0 增量 | E2E 链路通 + Goal 系统就绪 |
| **W7** | 可观测 + Feature Flags + Identity + Crash + AuthToken refresh | 3 周 | dasclaw_observability + dasclaw_features + dasclaw_identity + dasclaw_crash + dasclaw_net_proxy + AuthToken 续期 | — |
| **W8** | 验证与硬化 | 3 周 | 三库测试工具复用 + E2E + 契约测试全过 | 可演示 |
| **W9** | 文档 + 收尾 | 2 周 | ADR 全部归档 + 迁移指南 + 性能基准 | — |

---

## 通用 Wave 完成准则（v2.4 新增）

> **背景**：v2.3 之前各 Wave 验收只列 build/test/clippy，缺乏 commit 前的代码 review 环节。v2.4 起每个 Wave **任意一个大功能块（≥1 crate 的核心 trait 实现 / ≥300 LOC 新代码 / 涉及安全或 IPC 边界）** 完成后，**commit 之前**必须依次经过下述两步 skill 流程，结果记入该 Wave 验收 checklist。

### Step 1 — 写完即清理（轻量、必做）

应用 [`.github/skills/code-simplifier`](../../.github/skills/code-simplifier/SKILL.md)：

- 减嵌套、去冗余、改命名、合并相关逻辑
- 移除显而易见操作的注释
- **不改行为**，只改可读性
- 项目特化规则：Rust 顶层函数显式返回类型；禁止嵌套三元；显式优于紧凑

### Step 2 — 大块完成做严肃 review（必做）

应用 [`skills/code-review-expert`](../../skills/code-review-expert/SKILL.md)：

- 严重度：**P0 必须修才能 commit / P1 应当修 / P2 可创建 follow-up / P3 可选**
- 强项 4 维：**SOLID 违规 + 架构异味 + 安全风险 + 语言特化**（Rust → 自动加载 references/language-rust.md，覆盖 unwrap/clone 滥用、生命周期、Send/Sync、unsafe）
- 必须查：所有变更跨边界路径（auth / IPC / sandbox / approval / 数据写入 / 网络）
- **删除候选评估**：识别死代码、过度抽象、可移除的 feature flag

### 不用 `.codex/skills/code-review` 的原因

- 该 skill 与 code-review-expert 范围重叠，但**依赖 agent delegation（THOROUGH tier）**，调度成本高
- 严重度只有 4 级 + 通用清单，没有 SOLID 专项 / 语言专项 reference
- 结论：**code-review-expert 是 commit 前 review 的唯一首选**；code-review 仅在 expert 不可用时降级使用

### Wave 验收 checklist 通用项（每 Wave 自动叠加）

- [ ] 大功能块 commit 前已跑 code-simplifier
- [ ] 大功能块 commit 前已跑 code-review-expert，P0/P1 全部修复或显式接受 follow-up
- [ ] review 输出（findings 列表 + 修复说明）作为 commit message 一部分或 PR 描述

---

## W1 · 基线收敛（3 周）

### 进入条件
- [ ] 用户确认 31 文档 §8 的 D1-D10 决策点
- [ ] 31 架构文档评审通过
- [ ] **D1.5 三家 Agent Runtime 能力融合矩阵确认**（见下表，必须显式签字）

### D1.5 三家 Agent Runtime 能力融合矩阵

> 调研依据：codex-cli-main `core/src/session/turn.rs`、claw-code `runtime/src/conversation.rs`、ironclaw `agent/agentic_loop.rs`（已部分移植到 [crates/x_claw_agent/src/agentic_loop.rs](../../../crates/x_claw_agent/src/agentic_loop.rs)，Phase 3 Step D-4 Route B）。

**结论**：以 ironclaw `LoopDelegate` trait 为骨架（已在 x_claw_agent 验证可移植），从 claw-code 与 codex 各取增量。

| 能力 | 来源 | 当前状态 | W1 任务 |
|------|------|---------|--------|
| LoopDelegate trait + LoopSignal/LoopOutcome | ironclaw | ✅ 已移植到 x_claw_agent | 升级为 dasclaw_core 基线（保留 x_claw_agent 文件名，crate rename 推迟到 W6 收尾） |
| HookBundle / SafetyHook（Allow/Redact/Block + Error 上抛） | x_claw_agent Phase 3 Step D-4 | ✅ 已超越 ironclaw 上游 Fail-Open 缺陷 | 保留，**禁止回退到 ironclaw 上游 Fail-Open 语义** |
| Truncation force_text 兜底 | ironclaw | ✅ 已在 x_claw_agent | 保留 |
| 重复失败指纹追踪（DuplicateToolCallTracker）| ironclaw dispatcher.rs | ❌ 仅在 chat dispatcher，未上提到 LoopDelegate | **W3 上提**到 dasclaw_core，作为 trait 默认能力 |
| Compaction tool-use/tool-result 边界保护 | claw-code compact.rs:110 | ❌ 缺失 | **W4 移植**到 dasclaw_core |
| PreToolUse hook 三态（input override + permission override + cancel/deny）| claw-code | ⚠️ x_claw_agent SafetyHook 仅 Allow/Redact/Block，未含 input/permission override | **W3 扩展** SafetyDecision 枚举 |
| 并行 tool 的 RwLock 粒度模型（工具自声明 supports_parallel）| codex parallel.rs | ⚠️ ironclaw JoinSet 无粒度 | **W6** 引入 supports_parallel 标志 |
| 双阶段 Compaction（Pre-sampling + Mid-turn）| codex compact.rs | ⚠️ ironclaw 仅 Pre-turn | **W6** 增 Mid-turn 阶段 |
| Rollout Trace（RawEventSeq + wall_time，graceful degradation）| codex rollout-trace crate | ❌ ironclaw 仅 DB 落盘 | **W6** 直接 port codex rollout-trace 进 dasclaw_observability |
| Governance 6 件套 | claw-code | ❌ | ADR-104 已规划 → dasclaw_governance（W4） |
| session_budget_usd 真金白银预算 | ironclaw cost guard | ✅ 在 dispatcher 实现 | **W3 上提**到 LoopDelegate::before_llm_call 默认实现 |
| Recovery Recipes（7 种 FailureScenario）| claw-code recovery_recipes.rs | ❌ | **W4** port 到 dasclaw_governance |

> 完整对比报告参见对话 §"Agent Runtime 三家深度对比"。

### 任务（v2.3 订正：W1 不删 fork、不升级 0.26）

> **背景订正**：之前版本写 "删除 fork + 顶层 0.26 依赖" 与 ADR-101 §136 + 38 §137 矛盾。git log 实测 fork 在 0.26.0 tag 之后有 **42 fork-only commit**（Phase 2/3 dasclaw 接线核心成果），不能丢。

1. **保留 desktop-client/ironclaw fork**（W1-W6 期间作为私货来源）
   - **不**备份到 archive/、**不**删除、**不**改顶层依赖
   - desktop-client/Cargo.toml 维持现状（`ironclaw_safety` + `ironclaw_common` path dep）
   - 顶层 workspace.members 维持现状（含 `desktop-client/ironclaw` + 其 2 子 crate）
   - 删除时机：W6+ 私货全迁完后
2. **新建 crate 骨架**（无实现，仅目录 + Cargo.toml + lib.rs trait）
   - crates/dasclaw_core                ← 由 x_claw_agent 升级，文件保留原路径，crate name 在 W6 切换
   - crates/dasclaw_sandbox
   - crates/dasclaw_sandbox_windows    ← v2.5+ 新增（verbatim port from codex `windows-sandbox-rs`，见 ADR-129 / ADR-130；已实际落地 Phase 1.1.4j）
   - crates/dasclaw_pty                  ← v2 新增
   - crates/dasclaw_hooks
   - crates/dasclaw_apply_patch
   - crates/dasclaw_project_docs         ← v2 新增
   - crates/dasclaw_bash_validation      ← v2 新增
   - crates/dasclaw_governance
   - crates/dasclaw_mcp
   - crates/dasclaw_execpolicy
   - crates/dasclaw_features
   - crates/dasclaw_observability
   - crates/dasclaw_identity
   - crates/dasclaw_crash                ← v2 新增
   - crates/dasclaw_net_proxy            ← v2 新增
   - crates/dasclaw_safety（从 desktop-client/ironclaw/crates/ironclaw_safety 提升）
   - crates/dasclaw_common（从 desktop-client/ironclaw/crates/ironclaw_common 提升）
3. **dasclaw_core 雏形**（澄清：不是 v1↔v2 引擎切换）
   - 把 crates/x_claw_agent 内容（Step D-4 LoopDelegate Route B + HookBundle 完成）作为 dasclaw_core 起点
   - 吸收 desktop-client/ironclaw/src/agent/dispatcher.rs 的 chat 适配逻辑（重写为 ChatDelegate 实现 LoopDelegate）
   - **不引入 ironclaw_engine 0.26 v2 CodeAct**（桌面端无 Python 解释器场景，W6 评估是否激活）
4. **CI 模板**：每个新 crate 加 cargo build/test/clippy 流水线

### 产出
- 顶层 workspace 包含 12+ crate，全部 `cargo build --workspace` 通过
- desktop-client/ironclaw 子目录删除，README 加迁移说明

### 验收标准（必过）
- [ ] `cargo build --workspace` 0 错误 0 警告
- [ ] `cargo test --workspace` 全部通过（不含新 crate 的测试）
- [ ] desktop-client 启动后 chat 流水线可走通（基线不能回退）
- [ ] desktop-client/tests/tauri_command_contract_tests.rs 全过

### 回滚点
W1 失败：恢复 desktop-client/ironclaw 子模块（git revert）。

---

## W2 · Sandbox 三平台 + PTY + Workspace 策略层（5 周，v2.1 +1 周）

### 进入条件
- W1 验收过

### 任务
1. **从 codex-cli-main port sandbox**（参见 [35 §C](35-codex-capability-inventory.md)）：
   - codex-cli-main/codex-rs/linux-sandbox → dasclaw_sandbox/src/linux/
   - codex-cli-main/codex-rs/sandboxing（macOS seatbelt + .sbpl） → dasclaw_sandbox/src/macos/
   - codex-cli-main/codex-rs/windows-sandbox-rs/ → dasclaw_sandbox_windows/（verbatim，ADR-129） + dasclaw_sandbox/src/windows/（adapter 代 dasclaw_sandbox 调用 verbatim crate，ADR-130）。注：v2.5 之前计划为 `core/src/windows_sandbox.rs + windows_sandbox_read_grants.rs`，codex 上游已拆为独立 crate，dasclaw 跟进拆分。
   - codex-cli-main/codex-rs/process-hardening → dasclaw_sandbox/src/hardening.rs
2. **抽 trait**：`pub trait Sandbox { fn execute(&self, cmd: Command) -> Result<Output>; }`
3. **保留 ironclaw Docker 沙箱**作为高隔离备选实现（trait 多实现）
4. **PTY port**（v2 新增）：
   - codex-cli-main/codex-rs/exec-server / unified_exec → dasclaw_pty
   - portable-pty 抽 `pub trait Pty { spawn / write / read / resize / signal }`
   - 与 sandbox 集成：spawn 后进程仍在 sandbox 内部
5. **dasclaw_workspace_cap 策略层**（v2.1 新增，回应用户 Q1 + Q2）：
   - 移植 codex `protocol/src/protocol.rs` 的 SandboxPolicy enum 到 `dasclaw_workspace_cap::policy`
     - DangerFullAccess / ReadOnly / WorkspaceWrite / ExternalSandbox 四档
     - WritableRoot.read_only_subpaths 洞中洞机制（保护 `.git/hooks` `.codex` `.git`）
     - FS / Network 正交（每档独立 `network_access`）
   - 保留现有 cap-std kernel-level 强制层为最低基线
   - **范围限定**：仅 desktop-client 本地，不扩到 admin-backend（用户决策）
   - serde + JsonSchema + TS 三宏，desktop-client 前端 TS 类型直接消费
6. **集成到 dasclaw_core**：tool 执行必经 sandbox（即使是 NoopSandbox）+ 必经 dasclaw_workspace_cap policy 检查。终端类工具另走 PTY 路径。
7. **测试**：每个平台至少 1 个集成测试 + 失败路径测试（fail-safe：沙箱失败时拒绝执行而非允许）+ WritableRoot 洞中洞契约测试（写 `.git/hooks/pre-commit` 必须被拒绝）

### 产出
- dasclaw_sandbox + dasclaw_pty crate 可独立 cargo test
- dasclaw_workspace_cap 策略层 + cap-std 强制层 双层就位
- ironclaw 工具调用全部经过 dasclaw_sandbox + dasclaw_workspace_cap

### 验收
- [ ] 三平台单元测试通过（CI matrix 覆盖 ubuntu/macos/windows）
- [ ] 失败路径测试：沙箱不可用时工具调用被拒绝，不降级为直接 exec
- [ ] **WritableRoot 洞中洞契约测试**：sandbox 在 WorkspaceWrite 模式下写 `.git/hooks/*`、`.codex/*`、`.git/config` 必须返回 SandboxError，且无任何 fallback。**已知限制（v2.5 补充）**：当前仅在 `dasclaw_sandbox` adapter 层检查，内核层强制（Linux Landlock-based subpath deny / macOS Seatbelt subpath deny / Windows Restricted Token DACL）跟进补强由 [#324 sub-task 2](https://github.com/Linnanli/xClaw/issues/324) 追踪
- [ ] **ExternalSandbox 嵌套测试**：在 docker 容器内启动 desktop-client，agent 工具调用走 ExternalSandbox 模式，network_access 由 NetworkAccess 枚举决定
- [ ] 性能基准：进程沙箱启动 < 50ms（对比 codex 基线）
- [ ] PTY resize/信号转发 在 macOS+Linux 各 1 个示例脚本过
- [ ] dasclaw_workspace_cap 协议 enum 通过 `ts-rs` 导出 TypeScript 类型，desktop-client 前端可直接 import

### 复用 codex 测试
- codex-rs/linux-sandbox/tests/* 直接 port
- codex-rs/sandboxing/tests/* 直接 port
- codex-rs/exec-server/tests/* 直接 port
- codex-rs/protocol 的 SandboxPolicy 序列化测试 直接 port

---

## W3 · Hooks 引擎 + Apply Patch + 项目文档（含 E 类多层加载）（3 周）

> **前置门禁**：W3 启动前必须先完成 **W3-A Phase 0**（10 行红线 + 14×3 契合度评估）。  
> 评估方案见 [ADR-112](adr-112-compatibility-evaluation.md)；**评估嵌入节奏**（B1 同步补测 / B2 中段接 CI / B3 末尾收口）见 [ADR-112 §5.4](adr-112-compatibility-evaluation.md#54-评估嵌入节奏b1--b2--b3)。

### 任务
1. **dasclaw_hooks**：
   - 从 codex-cli-main/codex-rs/hooks port `schema.rs` / `registry.rs` / `engine.rs`
   - 合并 ironclaw 6 lifecycle 拦截点定义（BeforeInbound/BeforeToolCall/BeforeOutbound/OnSessionStart/OnSessionEnd/TransformResponse）
   - 合并 x_claw_agent 已完成的 hooks trait（SafetyHook/SandboxExecutor/SecretProvider/ApprovalGate）
2. **dasclaw_apply_patch**：
   - 从 codex-cli-main/codex-rs/apply-patch port 完整 lark 语法解析器
   - 暴露 `ApplyPatchTool` 注册到 ironclaw tools/builtin
3. **dasclaw_project_docs**（v2 新增 ADR-106 / **v2.1 E 类显式扩展**）：
   - 参考 codex `project_doc_max_bytes` 机制 + claw `ConfigLoader::default_for(cwd)` 多层加载
   - 优先级：AGENTS.md > CLAUDE.md > .codex/agents.md > project config
   - 多层合并：user 全局（`~/.config/dasclaw/AGENTS.md`） → project 项目级 → cwd 子目录覆盖
   - 递归向上查找直到 repo root 或 `$HOME`（参考 codex `agents_md.rs:367 LOC` 实现）
   - 提供 `ProjectDocLoader` trait：`load(cwd) -> Vec<ProjectDoc { content, source_path, layer, bytes }>`
   - 单层 max_bytes 默认 8KB（与 codex 对齐）超出按行截断而非报错
   - **加载机制**：SessionManager 在 session 创建时（OnSessionStart 时机点）**构建期 DI** 调用 `LayeredProjectDocLoader::load()` + `assemble_section()` 直接注入 system prompt 的 dynamic boundary 之后；**不进 `dasclaw_hooks` 系统**（与 codex/claw-code/ironclaw 三参考库一致；详见 [ADR-115](adr-115-project-docs-not-in-hook.md) + ADR-113 §2.4 反跨界原则）
   - **不采用 claw-code 单层方案**（[36 §7](36-claw-code-capability-inventory.md)）— claw 仅支持 cwd 单一 CLAUDE.md，弱于 codex/ironclaw
4. **集成到 dasclaw_core**：替代当前散落的 hook 调用点

### 验收
- [ ] hook 6 lifecycle 全部触发链路有 trace log
- [ ] apply_patch 通过 codex apply-patch 测试套件
- [ ] desktop-client SafetyBridge 通过 hooks 接入（替代当前直接调用）
- [ ] **E 类多层加载契约测试**：user/project/cwd 三层 AGENTS.md 合并顺序与 codex `agents_md.rs` 对齐（同一 fixture set）
- [ ] **递归向上查找单测**：在 cwd 嵌套 5 层场景下能正确收集所有 AGENTS.md 直到 repo root
- [ ] ProjectDoc 过大（> 8KB 默认 `project_doc_max_bytes`）时按行截断，不报错

### 复用 codex 测试
- codex-rs/hooks/tests/* port
- codex-rs/apply-patch/tests/fixtures/*.patch 全套用例
- codex-rs/config/tests/project_doc_*.rs port

---

## W4 · 治理 6 件套 + bash_validation（3 周）

### 任务
从 claw-code/rust/crates/runtime/src/ port 进 dasclaw_governance/src/：
- policy_engine.rs（PolicyCondition::GreenAt/StaleBranch/And/Or + PolicyAction::MergeToDev/RecoverOnce/Escalate）
- recovery_recipes.rs（FailureScenario 7 variants + RecoveryStep + escalation）
- trust_resolver.rs（OAuth + worktree + allowlist）
- branch_lock.rs
- stale_base.rs + stale_branch.rs（Fresh/Stale/Diverged + AutoRebase/MergeForward/WarnOnly/Block）
- green_contract.rs（level 0-3） + tools/lane_completion.rs
- lane_events.rs（与 governance 孔邑联动）

**v2 新增：dasclaw_bash_validation**（D10）：
- 从 claw-code/rust/crates/runtime/src/bash_validation.rs port（1004 LOC × 6 验证模块）
- 连接到 dasclaw_hooks::BeforeToolCall：bash 工具调用前必过验证

### 默认关闭策略
所有 6 件套默认通过 `dasclaw_features` 启用：
```toml
[features.disabled-by-default]
dasclaw_governance.policy_engine = false
dasclaw_governance.recovery_recipes = false
# ...
```

### 验收
- [ ] 18 unit + 6 integration test（claw-code 现有测试套）port 后通过
- [ ] 与 hooks 引擎集成：governance decision 通过 BeforeToolCall hook 注入

### 复用 claw-code 测试
- claw-code/rust/crates/runtime/tests/policy_*.rs
- claw-code/rust/crates/runtime/tests/recovery_*.rs
- claw-code/rust/crates/runtime/tests/branch_lock_*.rs

---

## W5 · MCP 统一 + ExecPolicy（2 周）

### 任务
1. **dasclaw_mcp**：以 claw-code/rust/crates/runtime/src/mcp_client.rs 为基线 port，6 transport 全部纳入：
   - Stdio / Sse / Http / WebSocket / Sdk / ManagedProxy
2. **dasclaw_execpolicy**：从 codex-cli-main/codex-rs/execpolicy port Starlark 引擎（**verbatim port，规约见 [ADR-132](adr-132-execpolicy-starlark-port-plan.md)**：starlark = "=0.13" exact-pin、公共 API 冻结、`scripts/check_codex_execpolicy_drift.py` guard）
3. ironclaw 现有 ManagedMcp 改为 dasclaw_mcp 的 adapter
4. **dasclaw_shell_command**（[ADR-133](adr-133-shell-command-adoption-eval.md) 决议 adopt-with-adapter，与本 Wave 同窗口落地）：从 codex-cli-main/codex-rs/shell-command verbatim port，作为 ironclaw UI 的 `ParsedCommand` 渲染源；与 `dasclaw_bash_validation`（门控）职责正交，**不**合并

### 不在本 Wave（明确 non-goal）
- ❌ codex `shell-escalation`（Unix-only setuid 拦截器）— 不 port、不 stub、不 vendor 调用，决议见 [ADR-134](adr-134-shell-escalation-non-goal.md)。dasclaw 桌面客户端走 governance + bash_validation + execpolicy + net_proxy + OS 原生 sandbox 五层组合，已覆盖 escalation 想解决的需求。

### 验收
- [ ] 6 transport 全部有 1 个端到端测试
- [ ] execpolicy 解析 codex 现有规则集（上游 `tests/basic.rs` verbatim port 33/33 `#[test]` 通过；ADR-132 §2.4 已修正为单文件 fixture，原"prefix_rule.rs + network_rule.rs"分文件抽样作废，门类覆盖不变）
- [ ] dasclaw_shell_command `parse_command` 端到端测试（codex 上游 fixture 选 prefix-match + powershell-detect 两组 verbatim 通过）

---

## W6 · 客户端外壳整合 + 后端基底吸收 + LLM provider 中立迁出（**5.5 周**，v2.5 +0.5 周）

> **v2.5 调整**：在 v2.4 5 周基础上 +0.5 周吸收任务组 C — `crates/dasclaw_llm_provider` 新建 + 删 `claw-code-api` path-dep（ADR-118）。
>
> **v2.2 调整**：原 4 周仅做 IPC 切换，扩为 5 周：吸收 ironclaw 后端能力（workspace 去多租户化 + bridge_lite 5-7k）+ IPC 整合 + 88 commit codex 增量跟进。

### 任务

#### 任务组 A — 后端基底吸收（v2.2 新增）

1. **dasclaw_workspace**（v2.2 调整）：
   - 移植 ironclaw `src/workspace/` + `src/db/libsql/workspace.rs` 共 12,857 LOC
   - **去多租户化**：砍掉 user_id 字段、ownership/、多用户隔离（约 -3k LOC）
   - **保留**：per-project + chunker + embeddings + schema + Hybrid Search RRF (k=60)
   - **目标产物**：≈ 7-9k LOC（**不是 12.8k**）
   - 验收：单项目场景下 chunker + RRF k=60 能召回正确 top-K
2. **dasclaw_bridge_lite**（v2.2 新增，ADR-110）：
   - 从 ironclaw `src/bridge/` 25,369 LOC 中精选移植：
     - `effect_adapter.rs` 中抽 EffectExecutor trait + safety/hook 链 (~2k)
     - `llm_adapter.rs` 1,595 LOC
     - `auth_manager.rs` 简化版 (~500)
     - `cost_guard_gate.rs` 41 + `user_facing_errors.rs` 448
   - **不要**：router 9.6k / store 大半 / skill_migration / 双模式适配
   - **目标产物**：≈ 5-7k LOC
3. **codex 88 commit 增量跟进**（v2.2 新增，详见 35 §Q）：
   - **goal 系统五件套**：goal_tool + goals.rs + thread_goal model + UI 适配 → 进 dasclaw_core
   - **ThreadStore trait 升级**：参考 codex thread-store crate 的 InMemory + LiveThread 设计
   - **permissions profiles 重构**：移除 legacy read-only modes，统一 profiles
   - **rollout-trace 新结构**：code_cell + protocol_event + thread + tool_dispatch（移到 W7 dasclaw_observability）
   - **Unix socket transport**：评估是否替换 Tauri 当前 IPC 通道（**P1 候选，可推迟**）

#### 任务组 B — desktop-client IPC 整合（原 W6 任务）

4. **desktop-client/src/ipc/* 切换到 dasclaw_core**
   - chat.rs / threads.rs / approval.rs / plan_mode.rs 等所有 14 个模块
   - 保持 IPC 命令名不变（契约测试不能破）
5. **DLP / SafetyBridge 通过 dasclaw_hooks 接入**（不再硬编码调用点）
6. **AGENTS.md 加载器注入 AppState**（ADR-106）：
   - 启动时调用 `ProjectDocLoader::load(cwd)`加载
   - 冲突时 user > project > cwd 优先级仅限 v1.0
7. **LSP IPC 补齐**（ADR-108）：
   - 新增 desktop-client/src/ipc/lsp.rs：4 命令 `ic_lsp_start / ic_lsp_stop / ic_lsp_query / ic_lsp_diagnose`
   - 同步更新 `all_tauri_commands!()` + `FRONTEND_INVOKED_COMMANDS`
8. **MCP IPC 补齐**（ADR-108）：
   - 新增 desktop-client/src/ipc/mcp.rs：6 命令 `ic_mcp_attach / detach / list_tools / call_tool / list_servers / health`
   - 依赖 dasclaw_mcp（6 transport）
9. **Approval WebSocket 升级**（ADR-107a）：
   - approval_polling.rs 重构为 `approval_subscribe.rs`：WebSocket 优先，30s polling 降级
   - 上游复用 ironclaw/channels/web 已有 axum WebSocket 能力
   - PendingTicketStore 保留，仅变更订阅机制
10. **Goal IPC 适配**（v2.2 新增）：
    - desktop-client/src/ipc/goal.rs：3 命令 `ic_goal_set / ic_goal_status / ic_goal_cancel`
    - 前端 GoalMenu UI（自建，不复制 codex tui 实现）
11. **删除中间桥接代码**（如有）
12. **engine_startup_tests.rs 重写**：覆盖新 runtime 启动时序 + ProjectDocLoader 初始化

#### 任务组 C — LLM provider 中立迁出（v2.5 新增，[ADR-118](adr-118-claw-code-readonly-and-self-impl.md)）

> **背景**：[31-target-architecture.md L139](31-target-architecture.md) 已陈述「`claw_code_provider.rs` 1334 行... 中立迁出」，但 v2.4 之前未列具体 task。ADR-117 v1.0 D5/D7 错误地将「在 claw-code 子仓内删除」作为收尾，已被 ADR-118 推翻。**正确做法**：claw-code 子仓**保持只读参考库**（不删原代码、不解绑 submodule），主仓**自实现** LLM provider crate，**删除** `desktop-client/ironclaw` 对 `claw-code-api` 的 path-dep。

13. **新建 `crates/dasclaw_llm_provider`** (~3.5-4.5k LOC)：
    - 参考 `claw-code/rust/crates/api/src/providers/{anthropic,openai_compat,mod}.rs` 实现，**不消费**
    - 主仓自定义类型：`AnthropicClient` / `OpenAiCompatClient` / `OpenAiCompatConfig` / `ProviderClient` enum / `AuthSource` / `ProviderKind` / 模型别名表 / `ApiError`
    - 消息类型：`InputContentBlock` / `InputMessage` / `OutputContentBlock` / `MessageRequest` / `MessageResponse` / `ToolDefinition` / `ToolChoice` / `ToolResultContentBlock` / `Usage`
    - SSE 解析（`parse_frame` / `SseParser`）
    - HTTP 客户端构造（`build_http_client` / `ProxyConfig`）
    - **契约测试**（mock HTTP server）：与 claw-code-api 行为等价
14. **重写 `desktop-client/ironclaw/src/llm/claw_code_provider.rs`**：
    - 改 `use claw_code_api::{...}` → `use dasclaw_llm_provider::{...}`
    - 内部映射逻辑（`build_chat_message_request` / `map_message_response` 等）保持不变
    - 文件可保留原名或改为 `dasclaw_llm_provider.rs`
15. **删 path-dep**：
    - `desktop-client/ironclaw/Cargo.toml:149` 删 `claw-code-api = { path = "../../claw-code/rust/crates/api", package = "api" }`
    - `desktop-client/ironclaw/Cargo.toml:235` 删 `claw-code-llm = []` no-op feature
    - 加 `dasclaw_llm_provider = { path = "../../crates/dasclaw_llm_provider" }` 依赖
16. **验证**：
    - `cargo check -p ironclaw -p dasclaw_llm_provider` 通过
    - `cargo nextest run -p ironclaw --tests llm::` 全过
    - `grep -rln "claw_code_api\|claw-code-api" desktop-client/ Cargo.toml crates/` 输出 = 空（除注释/历史 ADR 引用）

### 验收
- [ ] dasclaw_workspace **≤ 9k LOC**（含 RRF k=60 + chunker + embeddings，已去多租户化）
- [ ] dasclaw_bridge_lite **≤ 7k LOC**（不含 router/store_adapter/skill_migration）
- [ ] desktop-client/tests/tauri_command_contract_tests.rs 全部命令通过（含新增 LSP/MCP/Goal）
- [ ] desktop-client cypress E2E 全部通过
- [ ] DLP 8 命令行为零回退
- [ ] Approval WebSocket 路径 与 polling 降级路径两者都有 E2E 覆盖
- [ ] AGENTS.md 加载在启动时序中位于 EngineState::Ready 之前、ProjectDoc 注入 system prompt
- [ ] Goal 系统：用户可设"目标 + 预算"，超预算时 agent 暂停并通过 IPC 推送 GoalLimitReached 事件
- [ ] codex 88 commit 中 5 项 P0 增量已纳入或确认推迟（goal/ThreadStore trait/permissions profiles/rollout-trace/Unix socket）
- [ ] **(v2.5)** `crates/dasclaw_llm_provider` ≤ 4.5k LOC，contract test 与 claw-code-api 行为等价
- [ ] **(v2.5)** `desktop-client/ironclaw/Cargo.toml` 中 0 处 `claw-code-api` / `claw_code_api` 引用
- [ ] **(v2.5)** `cargo tree -p ironclaw` 输出不再包含 claw-code path crates
- [ ] **(v2.5)** claw-code 子仓代码 0 行变化（保持只读参考库）

---

## W7 · 可观测 + Feature Flags + Identity + Crash + Net Proxy + AuthToken Refresh（3 周）

### 任务
1. **dasclaw_features**：从 codex/features port 4 阶段生命周期（UnderDevelopment/Experimental/Stable/Deprecated/Removed）
2. **dasclaw_observability**：合并 ironclaw observability + codex rollout-trace
3. **dasclaw_identity**：从 codex agent-identity + device-key port
4. **dasclaw_crash**（v2 新增，ADR-109）：
   - `panic::set_hook` 统一安装，错误产出到本地 dump + 可选后端
   - 默认后端走 admin-backend `/api/client-reports`（复用 DataReporter）
   - 可插拔 sentry/datadog adapter
5. **dasclaw_net_proxy**（v2 新增）：从 codex/network-proxy port rama 框架 + HTTP_PROXY/HTTPS_PROXY/NO_PROXY + 自签 CA + MITM 拦截
6. **AuthToken refresh**（v2 新增，ADR-107）：
   - 从 codex/login crate port `refresh_token` 流
   - desktop-client/src/auth_token_manager.rs 增加后台定时刷新任务（到期前 5 min）
   - 失败路径：连续刷新 N 次失败 → 提示重新登录

### 验收
- [ ] 所有新 crate 都接入 feature flag（默认值符合 31 文档约定）
- [ ] rollout trace 在测试场景下能产出推理链路 dump
- [ ] panic hook 人为触发 panic 能准确产出本地 dump + admin 上报
- [ ] AuthToken 在 mock OAuth server 上能自动完成 refresh 循环
- [ ] HTTP_PROXY 环境变量生效，可访问代理后的 LLM endpoint

---

## W8 · 验证与硬化（3 周）

### 任务
1. **复用三库测试工具**（详见 33 文档）
2. **新增契约测试**：dasclaw_sandbox / dasclaw_hooks / dasclaw_governance trait 都加 trait 契约测试
3. **失败路径测试**：所有"不可绕过"路径加 fail-safe 测试
4. **性能基准**：建立 baseline（推理延迟、沙箱启动、tool 调用）
5. **PICT 测试矩阵**（参考 AGENTS.md）：sandbox × DLP × hooks × governance 组合场景

### 验收
- [ ] cargo-llvm-cov 覆盖率：dasclaw_* crate 单元 >90% / 安全模块 100%
- [ ] DLP 失败路径覆盖率 100%
- [ ] PICT 设计矩阵覆盖到所有 P0 风险组合

---

## W9 · 文档 + 收尾（2 周）

### 任务
1. **ADR 归档**：ADR-101 ~ ADR-105 全部 commit
2. **迁移指南**：写 desktop-client/docs/MIGRATION_FROM_FORK.md
3. **新人 onboarding**：30/31/32/33 + ADR + cleanup-proposal 索引
4. **清理 architecture-refactor 旧文档**（按 cleanup-proposal.md 执行）
5. **README 更新**：x-claw 总仓库说明四方融合最终态

### 验收
- [ ] 全部 ADR 提交
- [ ] 全部历史文档归档或删除
- [ ] 新 README 索引清晰

---

## 跨 Wave 风险登记

| 风险 | 等级 | 缓解 |
|------|-----|------|
| ironclaw-main 0.26 仍在演进，主分支变动频繁 | 🔴 高 | W1 锁定具体 git tag，每月 sync 一次 |
| codex sandbox port 时三平台 CI 不全 | 🟡 中 | W2 提前在 GitHub Actions 配 ubuntu/macos/windows matrix |
| claw-code 6 件套 port 后 trait 设计与 hooks 不兼容 | 🟡 中 | W3 完成 hooks 后 W4 才开工，trait 提前 review |
| desktop-client IPC 契约破裂导致 React UI 崩 | 🔴 高 | W6 每天跑 contract test + cypress E2E |
| LSP/MCP IPC 新增后 前端未接入 | 🟡 中 | W6 同步更新前端调用点，加到 cypress 覆盖 |
| Approval WebSocket 与 polling 双路径状态机不同步 | 🟡 中 | W6 在 PendingTicketStore 保证两者幂等 |
| AGENTS.md 加载在 EngineState::Ready 前完成，启动超时风险 | 🟡 中 | W3 加载设 2s 超时，超时后降级为空文档并警告 |
| 性能回退（沙箱开销） | 🟡 中 | W8 基线对比，超过 10% 触发 review |

---

## 并行机会

| 阶段 | 可并行 |
|------|------|
| W2 + W3 | sandbox 与 hooks 弱耦合，可分两人 |
| W4 + W5 | governance 与 mcp 各自独立 |
| W7 三个 crate | features / observability / identity 独立 |

**串行约束**：W1 → W2/W3 → W4 → W6 → W8 → W9

---

## 与 27 文档的差异

27 文档"架构优先"方向（A1-A9 共 8.1 月）→ 本文档"9 Wave 共 6-8 月"

| 27 任务 | 32 对应 |
|---------|--------|
| A1 架构骨架 + 3 crate + ADR | W1 + 跨 W2-W7 的 10 crate（更彻底） |
| A2 架构收口 | W6 |
| A3 客户端 E2E | W6 + W8 |
| A4 国产 LLM 适配 | 不在本计划（已经通过 ironclaw llm 完成） |
| A5 MVP 5 skill | 不在本计划（已存在） |
| A6 两条 demo E2E | W8 |
| A7 沙箱 facade | W2 |
| A8 DLP 输出侧基础 | 不需要新建（已有，W6 整合） |
| A9 凭证隔离 | W7（identity） |
