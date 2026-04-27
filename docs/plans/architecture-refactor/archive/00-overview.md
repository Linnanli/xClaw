# x-claw 架构重构方案总览

> **状态**：Draft — 2026-04-20
> **作者**：Claude + @nallylin
> **方案版本**：v1.0（方案 B：决策 + 实施清单）

---

## 目标

让 x-claw 在单一代码库内同时拥有三个能力，且三者职责清晰不重叠：

1. **claw-code 的解题能力** — claude-code CLI 的 Rust 版本，保持从上游 rebase 新能力
2. **ironclaw 的完整安全方案** — DLP、沙箱、密钥、网络边界、工具审批
3. **客户端顺滑的 GUI 体验** — Tauri + React，对接 assistant-ui 原生能力，不重造消息/状态/审批

---

## 非目标

- **不重写 agent 语义**：保留 claw-code 的 agent loop、subagent、compaction、plan mode 等原生语义
- **不削减安全能力**：5 块安全能力（DLP/沙箱/密钥/网络/审批）全部保留，一个都不动
- **不追求"完全不用第三方库"**：目标是"成熟库就用成熟的，但只用一次、用在正确的层"

---

## 当前问题概述（详见 `01-current-gaps-analysis.md`）

三层错位：

| 层 | 应该是什么 | 实际是什么 | 代价 |
|----|----------|----------|------|
| **LLM 协议层** | rig-core 作为 Provider 抽象 | rig-core 被当"HTTP 客户端"用，还要打 JSON 补丁 | [`rig_adapter.rs`](../../desktop-client/ironclaw/src/llm/rig_adapter.rs) 2026 行 |
| **前端 Runtime 层** | assistant-ui/react 管消息/工具/审批 UI | 只当哑渲染容器，所有状态自己维护 | [`TauriRuntimeProvider.tsx`](../../desktop-client/src-ui/src/app/runtime/TauriRuntimeProvider.tsx) 1539 行 |
| **Agent 能力层** | 对齐 claw-code，能 rebase 上游 | 复制代码后散到 `agent/*` 24041 行，抽象与 claw-code 不一致 | 失去 rebase 能力 + 状态管理重复 3 份 |

---

## 目标架构概述（详见 `02-target-architecture.md`）

```
Frontend (assistant-ui 原生 useChatRuntime)
      ↓ Vercel AI SDK Data Stream Protocol
Tauri Transport (薄壳，~50 行协议转换)
      ↓
crates/x_claw_agent (fork claw-code runtime + commands，加 Safety Hook)
      ↓ hook 注入
crates/ironclaw_safety   ← 保留
crates/ironclaw_sandbox  ← 从 src/sandbox 抽出
crates/ironclaw_secrets  ← 从 src/secrets 抽出
crates/ironclaw_auth     ← 保留
      ↓
claw-code/rust/crates/api (作为 Provider 抽象，支持 Anthropic + OpenAI-compat)
```

---

## 分阶段路线图

| Phase | 名称 | 时间估算 | 风险 | 收益 | 文档 |
|-------|------|---------|------|------|------|
| 0 | 止血期 | 已完成 | 无 | Qwen 可用 + 审批卡片可见 | — |
| 1 | AI SDK 协议迁移 | 1-2 周 | 低 | 前端 1539 → ~400 行；SDK 原生接管审批/branch | [`03-phase1-ai-sdk-migration.md`](./03-phase1-ai-sdk-migration.md) |
| 2 | rig-core 下线 | 2-3 周 | 中 | 去掉 2026 行补丁代码；reasoning_content 等兼容问题根治 | [`04-phase2-claw-code-api.md`](./04-phase2-claw-code-api.md) |
| 3 | Agent 能力抽离 | 1-2 周 | 中 | 获得 claw-code rebase 能力；消除 agent 状态重复 | [`05-phase3-agent-extraction.md`](./05-phase3-agent-extraction.md) |
| 持续 | 上游同步 | — | 低 | 跟进 claude-code 新特性 | — |

**强烈推荐按顺序执行**。Phase 1 做完你立刻能看到前端 bug 率下降；Phase 2 之前不宜做 Phase 3，否则 agent 层和 LLM 层要改两次。

---

## 安全能力在每个阶段的保护策略（详见 `06-safety-preservation.md`）

| 安全能力 | Phase 1 | Phase 2 | Phase 3 |
|---------|---------|---------|---------|
| DLP / Prompt injection 防御 | ✅ 不动 | ✅ 不动 | ✅ 不动，作为 Hook 注入到 agent |
| 沙箱 / Docker 隔离 | ✅ 不动 | ✅ 不动 | ✅ 抽成独立 crate，作为 SandboxExecutor trait |
| 密钥管理 / Keychain | ✅ 不动 | ✅ 不动 | ✅ 抽成独立 crate |
| 网络边界 / Bearer token | ✅ 不动 | ✅ 不动 | ✅ 不动 |
| 工具审批 / 策略引擎 | ⚠️ 前端 UI 用 SDK 原生重写，后端规则引擎不动 | ✅ 不动 | ⚠️ 审批 Hook 接入新 agent crate |

每个阶段都有回归测试门禁，详见 [`07-rollback-plan.md`](./07-rollback-plan.md)。

---

## 关键决策摘要

**已决策**：

1. ✅ **rig-core 下线**，用 [`claw-code/rust/crates/api`](../../claw-code/rust/crates/api) 替代
   - 原因：rig-core 层级错位，结构固定扛不住新模型
   - 后果：OpenAI / xAI / DashScope / Kimi / Ollama 全部继续支持（已在 claw-code api 里）

2. ✅ **assistant-ui 切换到 `useChatRuntime`**，不再用 `useExternalStoreRuntime`
   - 原因：SDK 原生能力没用上，自己实现的质量又不够（branch 吞审批就是证据）
   - 前提：后端需输出 AI SDK Data Stream Protocol 标准帧

3. ✅ **新增 `crates/x_claw_agent`**，fork 自 claw-code 的 `runtime` + `commands`
   - 原因：保留 claw-code rebase 能力；消除 agent 状态重复管理
   - 关键设计：通过 `SafetyHook` / `SandboxExecutor` / `SecretProvider` trait 注入 ironclaw 安全能力

4. ✅ **ironclaw 的 `safety` / `sandbox` / `secrets` 保持独立 crate**
   - 原因：已做对；让 `x_claw_agent` 可单独发布/使用

**待决策**：

- ⏳ **`x_claw_agent` 与 claw-code 的同步策略**：
  - 方案 A：作为 git subtree，定期 `git subtree pull`（推荐）
  - 方案 B：完整 fork，手动 cherry-pick
  - 方案 C：把 claw-code 作为 submodule
  - **推荐方案 A**，Phase 3 开始时确定

- ⏳ **AI SDK 版本**：当前 `package.json` 是 `ai: ^4.0.0`，AI SDK v5 已发布
  - 方案 A：Phase 1 保持 v4，Phase 1.5 升级 v5
  - 方案 B：Phase 1 直接用 v5
  - **推荐方案 A**，减少一次性变更面

---

## 成功标准

每个 Phase 完成时必须满足：

- [ ] `cargo build -p desktop-client --lib` 0 错误 0 警告
- [ ] 前端 `npx tsc --noEmit` 对本次改动文件 0 错误
- [ ] 回归测试：发送一条含工具调用（`web_search` + `web_fetch` + `write_file`）的聊天，正常走完 approval → 执行 → 结果展示
- [ ] 安全冒烟测试：DLP 脱敏生效、沙箱拦截 `rm -rf /` 生效、审批卡片可见
- [ ] 代码行数指标达标（每个 phase 文档有具体数字）

---

## 文档索引

- [`01-current-gaps-analysis.md`](./01-current-gaps-analysis.md) — 详细问题分析（含 grep 证据 + 行数）
- [`02-target-architecture.md`](./02-target-architecture.md) — 目标架构详解 + crate 拆分 + Hook trait 设计
- [`03-phase1-ai-sdk-migration.md`](./03-phase1-ai-sdk-migration.md) — Phase 1 实施清单
- [`04-phase2-claw-code-api.md`](./04-phase2-claw-code-api.md) — Phase 2 实施清单
- [`05-phase3-agent-extraction.md`](./05-phase3-agent-extraction.md) — Phase 3 实施清单
- [`06-safety-preservation.md`](./06-safety-preservation.md) — 安全能力不破坏保证书
- [`07-rollback-plan.md`](./07-rollback-plan.md) — 每阶段回滚策略 + 测试门禁
