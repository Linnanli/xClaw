# Phase 4 — claw-code 开发者能力移植规划

> **版本**: v1.0 | **日期**: 2026-04-21 | **状态**: 规划 / 待评审
> **前置文档**: [claw-code-integration-plan.md](../claw-code-integration-plan.md) · [claude-code-parity-architecture.md](../claude-code-parity-architecture.md) · [UPSTREAM_BASELINE.md](../../../crates/x_claw_agent/UPSTREAM_BASELINE.md)
> **前提**: Phase 3 已 CLOSED（[05b](./05b-phase3-execution-plan.md)）

---

## 一、目标

把 claw-code 上游的**开发者工具链能力**移植到 x-claw，补齐 Phase 3 有意搁置的 parity 缺口，让 x-claw 从「政企级 AI 助手」真正成为「政企级 AI **编程**办公助手」。

**不是目标**：
- 不追求 100% API parity — 行为 parity 优先（继承 [claude-code-parity-architecture.md](../claude-code-parity-architecture.md) 的 P5 原则）
- 不把 ironclaw 现有 54 工具替换成 claw-code 40 工具 — **两者并存，按领域分工**
- 不动多通道 / DLP / RBAC / Docker 沙箱 / WASM 沙箱 / 审计等 ironclaw 差异化基座

---

## 二、能力差距盘点（基于 Phase 3 收口状态）

| 优先级 | claw-code 能力 | 上游位置 | 上游行数 | x-claw 现状 | 产品价值 |
|---|---|---|---|---|---|
| **P0** | 精细文件操作（`read_file` 偏移/limit、`edit_file` 精确替换、`glob`、`grep -B/-A/-C`） | `runtime/file_ops.rs` | ~1800 | ❌ 零 | 编程场景必备 |
| **P0** | Bash 验证引擎激活（只读检测、破坏命令警告、沙箱感知） | `runtime/bash_validation.rs` | 1004 | ✅ 代码已 port 进 `x_claw_agent/bash_validation.rs`，**未接入 dispatcher call path** | 安全必备 |
| **P1** | LSP 集成（定义跳转、引用查找、符号、诊断、悬停） | `runtime/lsp_client.rs` + `tools` | ~500 | ❌ 零 | 编程体验关键 |
| **P1** | Git 深度集成（stale base 检测、commit 上下文注入、diff 展示） | `runtime/stale_base.rs` + `git_context.rs` | ~800 | ❌ 零 | 工作流关键 |
| **P1** | 开发者 Slash Commands（/review /diff /commit /pr /bughunter /security-review） | `commands/*` | ~1500 | ❌ 零（ironclaw `agent/commands.rs` 是另一套） | 习惯驱动 |
| **P2** | Session Fork（从任意 Turn 分叉会话） | `runtime/session_control.rs` | ~400 | ❌ 零（ironclaw Thread 模型近似但不支持 fork） | 高级用户 |
| **P2** | Plan Mode / Exit Plan Mode（显式规划-执行切换） | `tools/EnterPlanMode` + `ExitPlanMode` | ~200 | ❌ 零 | 复杂任务必备 |
| **P2** | TaskPacket 结构化任务（目标/范围/验收标准/提交策略） | `runtime/task_packet.rs` + `task_registry.rs` | ~1400 | ❌ 零（ironclaw `routines` 作用不同） | 政企审批场景契合 |
| **P3** | Notebook 编辑（Jupyter 单元格操作） | `tools/NotebookEdit` | ~300 | ❌ 零 | 数据科学场景 |
| **P3** | Recovery Recipes（recipe 驱动修复） | `runtime/recovery_recipes.rs` | 633 | ❌ 零（ironclaw `self_repair.rs` 是另一实现） | Self-healing 增强 |

**不移植**（明确排除）：
- `policy_engine.rs` / `green_contract.rs` / `stale_branch.rs`（~1700 行）— 上游 worktree 多 lane 编排，与 ironclaw 单工作区流程不匹配
- `sandbox.rs` 上游 Linux namespace 描述符（385 行）— ironclaw 有更强的 DockerSandbox + Phase 4 engine v2 分层策略
- `bootstrap.rs` / `worker_boot.rs`（~1800 行）— ironclaw 有自己的 `engine_startup_tests` + `main.rs`
- `oauth.rs`（603 行）— provider 特化，不是 agent 层职责
- `mcp_*.rs` 8 文件（~6500 行）— ironclaw 已有 MCP 集成（`src/mcp/`），不复用上游
- `hooks.rs` 子进程扩展点（1116 行）— 与 ironclaw 内置 hook 概念冲突，走 WASM 扩展路径

---

## 三、对当前架构的调整影响评估

**核心问题：影响大吗？**

**答案：按 P0/P1 做，影响中等且可控；P2/P3 影响较大，需要单独决策。**

### 3.1 架构层级影响矩阵

| 架构层 | P0 影响 | P1 影响 | P2 影响 | P3 影响 |
|---|---|---|---|---|
| **`x_claw_agent` kernel** | 新增 `file_ops` 模块；bash_validation 接线 | 新增 `lsp_client` / `git_context`（可选：单独 crate） | 改 `session` 模型支持 fork；新增 `plan_mode` 状态机 | 低 |
| **`ironclaw` 工具注册表** | 新增 6 个工具到 `ToolRegistry` | 新增 10+ 工具 + LSP 客户端注入 | 新增 Plan Mode 状态 | 新增 NotebookEdit |
| **Hook 接线** | 需要把 `before_tool_call` / `after_tool_output` 接进 dispatcher（Phase 3 延期项） | 同上 | Approval 流程需支持 Plan Mode 预览 | 无 |
| **安全管道**（DLP / 审批 / 沙箱） | 新工具必须纳入 DLP 扫描 + 审批 + Docker 沙箱 | 同上，LSP/Git 输出也要 DLP | Plan Mode 预览阶段不走审批 | 同上 |
| **前端 UI** | Tool Result Renderer 需要 `edit_file` diff 可视化 | 需要 Git Panel / LSP 面板 / Plan Board | 需要 Plan Mode 切换按钮 + Session Fork 入口 | 需要 Notebook 编辑器 |
| **DB schema** | 无变化 | Git commit 上下文缓存（可选） | **Thread 表需加 `parent_thread_id` + `forked_at_turn` 列** | 无 |
| **Channels (Telegram/Slack/...)** | 新工具自动继承现有渠道 | 同上 | Plan Mode 需要渠道支持预览态 | 同上 |
| **Admin Backend** | 可能需要新增工具白名单配置 | 需要 LSP server 白名单 | 无 | 无 |

### 3.2 对 Phase 3 已完成成果的影响

| Phase 3 产物 | Phase 4 是否破坏 | 说明 |
|---|---|---|
| `x_claw_agent::run_agentic_loop` 内核循环 | ❌ 不破坏 | 新工具通过 `LoopDelegate::execute_tool_calls` 进入，不动 loop 主干 |
| 4 个 host Trait（SafetyHook / SandboxExecutor / SecretProvider / ApprovalGate） | ❌ 不破坏 | 新工具直接挂在 trait 之上；P0/P1 都不需要新增 trait |
| `HookBundle` 聚合 | ⚠️ 可能扩展 | Phase 4 engine v2 把 `SandboxExecutor` 真正接线时会用到 |
| `ironclaw_safety::agent-hook` feature | ❌ 不破坏 | 新工具输出走同一个 `after_tool_output` 审查路径 |
| `ironclaw/src/routines/` 顶级模块 | ❌ 不破坏 | TaskPacket（P2）可能在 routines 旁新增兄弟模块，不动 routines 本身 |
| cap-std 路径隔离 | ✅ 反而利好 | `file_ops` 的 workspace 边界直接复用 `cap_std::fs::Dir` |
| `agent/dispatcher.rs` / `agent_loop.rs`（Phase 3 保留） | ⚠️ 会改动 | Hook 三接线 + 新工具注册点都在这里 |

**结论：P0/P1 在 Phase 3 trait seam 之上增量开发；不回滚、不破坏任何已提交成果。**

### 3.3 风险项

| 风险 | 等级 | 缓解 |
|---|---|---|
| 新工具绕过 DLP（像旧 DLP 失败路径教训） | 🔴 高 | 每个新工具的 `execute_tool_calls` 必须补 `before_tool_call` + `after_tool_output` 契约测试（参考 `DLP_TESTING_LESSONS_LEARNED.md`） |
| LSP 长连接 / 子进程管理泄漏 | 🟡 中 | 在 `x_claw_agent::lsp_client` 里用 RAII + `Arc<Mutex<LspSession>>`；生命周期挂在 `SessionManager` |
| Git 操作触发敏感信息 leak（commit message 含密钥） | 🔴 高 | Git 工具的 `after_tool_output` 必须走 SafetyLayer 的 leak_detector |
| Session Fork 导致 thread 表空间爆炸 | 🟡 中 | 默认 fork 走 "copy-on-write"（新 thread 引用父 thread 快照），定期清理无活动 fork |
| Plan Mode 预览阶段误操作真实文件 | 🔴 高 | Plan Mode 强制走 `NoopSandboxExecutor` 或专用 dry-run executor |
| 上游 rebase 困难（claw-code 自身还在演进） | 🟡 中 | 严格按 porting log 机制，每次 cherry-pick 必留条目；不用 `subtree pull` |

---

## 四、建议分期推进

### Phase 4.0（建议，2-3 周）— P0 硬核

- **W1**: `x_claw_agent::file_ops` 模块（read/write/edit/glob/grep 五工具）+ `cap-std` 路径验证 + 10 个契约测试
- **W2**: ironclaw `ToolRegistry` 注册新工具 + Hook 接线（`before_tool_call` / `after_tool_output`） + DLP 集成测试
- **W3**: `bash_validation.rs` 激活到 dispatcher call path（Step K 的真正完成）+ 前端 Tool Result Renderer 的 diff 可视化

**退出条件**：
- `grep_search` / `edit_file` 通过 Web Gateway + Desktop Client + Telegram 三端都能跑
- DLP 扫描对新工具输出 100% 生效
- `cargo test -p x_claw_agent --lib` / `ironclaw --lib` 通过

### Phase 4.1（建议，3-4 周）— P1 编程体验

- **W1-W2**: LSP client 移植 + tower-lsp 集成 + 诊断/定义/引用工具注册
- **W3**: Git 深度集成（stale base / commit 上下文 / diff）
- **W4**: 开发者 Slash Commands（/review /diff /commit /pr）+ 前端 Git Panel

**退出条件**：
- 能在 Rust 项目里跑通"查看错误→定位定义→编辑修复→查看 diff→提交"完整流
- Admin Backend 能配置 LSP server 白名单

### Phase 4.2（可选，2-3 周）— P2 高级特性

- Plan Mode 状态机 + 前端 Plan Board
- Session Fork（thread 表 schema 迁移 + copy-on-write）
- TaskPacket（与 routines 并存，作为政企审批场景的结构化任务载体）

**前置决策**：产品端确认这些高级特性确实有客户需求，否则建议跳过。

### Phase 4.3（可选）— P3 长尾

- NotebookEdit（如果有数据科学客户）
- Recovery Recipes（如果 self_repair 需要增强）

---

## 五、关键决策点

**在启动 Phase 4.0 前，需要确认：**

1. **产品定位**：x-claw 是"政企级 AI 编程助手"还是"政企级 AI 办公助手"？
   - 前者：Phase 4.0/4.1 必做
   - 后者：只做 P0 里的 `edit_file` + `grep_search` 即可

2. **资源投入**：Phase 4.0/4.1 合计 5-7 周，期间不做新功能
   - 是否接受？
   - 是否需要其他模块（admin-backend 新功能 / 新渠道接入）并行？

3. **Hook 三接线**（`SandboxExecutor` / `SecretProvider` / `ApprovalGate`）：
   - 方案 A：Phase 4.0 先不接，继续用 ironclaw 现有 DockerSandbox + SecretsStore 直连
   - 方案 B：Phase 4.0 一并接线（多 1 周工作量，但拿到 Phase 3 真正的闭环）
   - 建议：**方案 A**（先产出用户可见能力，Hook 接线留到 engine v2 时一并做）

4. **`bash_validation` 激活的副作用**：
   - 上游是"pre-exec 文本验证"，叠加在 ironclaw DockerSandbox 之上
   - 可能会对现有 bash 命令误报（如 `rm -rf /tmp/xxx`）
   - 建议：上线初期只记录 warning，不拦截；灰度两周后转拦截

---

## 六、与现有文档的关系

- [claw-code-integration-plan.md](../claw-code-integration-plan.md) — **修订**：Phase 4 重新激活，P0/P1/P2 优先级与本文一致，工作量估算更精确
- [claude-code-parity-architecture.md](../claude-code-parity-architecture.md) — **部分接受**：采纳 P5（行为 parity）和 P2（安全管道不开后门）原则；**不采纳** P1（原生集成非桥接）中"淘汰 `JobMode::ClaudeCode` 外部 CLI 桥接"的表述 — 桥接模式作为向后兼容继续保留
- [05b-phase3-execution-plan.md](./05b-phase3-execution-plan.md) — **未破坏**：Phase 3 已 CLOSED；Phase 4 在 trait seam 之上增量开发
- [ADR-002](./adr-002-sandbox-backend-layered-strategy.md) — **继承**：Phase 4 engine v2 沙箱与本文 Phase 4.0/4.1 可**并行**，engine v2 里程碑不阻塞 file_ops / LSP 上线

---

## 七、下一步

**用户决策点**：

- [ ] 确认产品定位（编程 vs. 办公 vs. 两者）
- [ ] 确认 Phase 4.0 启动时机（立即 / 先做别的 / 暂缓）
- [ ] 确认 Hook 接线方案 A/B
- [ ] 确认 Phase 4.2 是否纳入 roadmap

完成决策后，为 Phase 4.0 建立详细的 W1-W3 执行计划（对标 05b 的粒度）。
