# 39 — desktop-client/ironclaw fork 私货清单与迁移地图

> **v1.0 (2026-04-26)** · 配套 [31](31-target-architecture.md) v2.3 / [32](32-execution-plan.md) v2.3 / [38](38-desktop-client-ironclaw-fork-inventory.md)。
> 目的：在 W6+ 删 fork 之前，明确 42 个 fork-only commit 留下的所有私货必须迁到何处，逐项打勾才能删 fork。
> 数据来源：`git log ironclaw-v0.26.0..HEAD --no-merges`（fork 在 0.26 tag 后的提交）+ `git diff --shortstat ironclaw-v0.26.0..HEAD`。

---

## 0. 总体差异

| 维度 | 数值 | 说明 |
|------|------|------|
| fork-only commit 数 | **42** | git log 实测 |
| 总文件数变更 | 1,209 | git diff --shortstat |
| 总行数 +/− | **+51,842 / −326,956** | 删除多 = fork 是 0.26 精简版（去多通道 / 大量外壳） |
| 独家新增文件（git diff --diff-filter=A） | **49** | 见 §1 |
| 修改上游文件（git diff --diff-filter=M） | **93** | 见 §2 |
| 独家文件总行数 | **≈ 14,800 LOC** | 见 §1 累计 |

> ⚠️ 这 42 commit 是 Phase 2/3 dasclaw 接线的全部成果，是吸收式重构的核心私货。删 fork 之前必须 100% 迁出。

---

## 1. 独家新增文件 → dasclaw_* 归属表（49 个 / ≈14,800 LOC）

### 1.1 LLM 层 → `dasclaw_core::llm`（建议）

| fork 路径 | LOC | 目标位置 | 备注 |
|-----------|---:|----------|------|
| src/llm/claw_code_provider.rs | 1,334 | dasclaw_core::llm::claw_code | Claude Code 协议适配主入口 |
| src/llm/openai_streaming.rs | ? | dasclaw_core::llm::openai_streaming | OpenAI 流支持 |
| src/llm/prompt/mod.rs | 260 | dasclaw_core::llm::prompt | Prompt 分层架构入口 |
| src/llm/prompt/static_layer.rs | 169 | 同上 | 静态层 |
| src/llm/prompt/dynamic_layer.rs | 164 | 同上 | 动态层 |
| src/llm/schema_utils.rs | 217 | dasclaw_core::llm::schema_utils | Schema 工具 |
| src/testing/scripted_llm.rs | 207 | dasclaw_core::testing | 脚本化 mock LLM |
| **小计** | **≈ 2,351** | | |

### 1.2 Agent runtime 接线层 → 多归宿

| fork 路径 | LOC | 目标位置 | 备注 |
|-----------|---:|----------|------|
| src/agent/traits_impl.rs | 125 | desktop-client/src/agent/traits_impl.rs | **Tauri 适配，不进 dasclaw_***（保留客户端壳层） |
| src/sandbox/agent_executor.rs | 277 | dasclaw_sandbox::adapters::ironclaw_executor | x_claw_agent::SandboxExecutor 实现 |
| src/secrets/agent_provider.rs | 205 | dasclaw_identity::adapters::ironclaw_secrets | x_claw_agent::SecretProvider 实现 |
| crates/ironclaw_safety/src/agent_hook.rs | 225 | **dasclaw_safety**（提升） | IronclawSafetyHook 适配器 |
| **小计** | **832** | | |

### 1.3 工具层（22 个 / 5,915 LOC）

#### 1.3.1 Bash 验证 → `dasclaw_bash_validation`

| fork 路径 | LOC | 目标位置 |
|-----------|---:|----------|
| src/tools/builtin/bash_validator.rs | 657 | dasclaw_bash_validation::validator |

#### 1.3.2 文件防护 → `dasclaw_safety`

| fork 路径 | LOC | 目标位置 |
|-----------|---:|----------|
| src/tools/builtin/file_guard.rs | 187 | dasclaw_safety::file_guard |

#### 1.3.3 代码编辑 → `dasclaw_apply_patch`（替换）

| fork 路径 | LOC | 目标位置 | 备注 |
|-----------|---:|----------|------|
| src/tools/builtin/code_edit.rs | 438 | dasclaw_apply_patch::compat | **用 codex apply_patch 协议替换**（30 §B 已决） |

#### 1.3.4 LSP 工具 → `dasclaw_lsp`（**31 蓝图缺口，建议补**）

| fork 路径 | LOC | 目标位置 |
|-----------|---:|----------|
| src/tools/builtin/lsp/tool.rs | 711 | dasclaw_lsp::tool |
| src/tools/builtin/lsp/client.rs | 403 | dasclaw_lsp::client |
| src/tools/builtin/lsp/mod.rs | 227 | dasclaw_lsp::lib |
| src/tools/builtin/lsp/server_config.rs | 179 | dasclaw_lsp::server_config |
| src/tools/builtin/lsp/protocol.rs | 174 | dasclaw_lsp::protocol |
| **小计** | **1,694** | |

#### 1.3.5 Git 工具集 → `dasclaw_git_tools`（**31 蓝图缺口，建议补**）

| fork 路径 | LOC | 目标位置 |
|-----------|---:|----------|
| src/tools/builtin/git/stale.rs | 212 | dasclaw_git_tools::stale（去 31 §B GitStaleCheckTool） |
| src/tools/builtin/git/branch.rs | 201 | dasclaw_git_tools::branch |
| src/tools/builtin/git/commit.rs | 166 | dasclaw_git_tools::commit |
| src/tools/builtin/git/log.rs | 165 | dasclaw_git_tools::log |
| src/tools/builtin/git/diff.rs | 158 | dasclaw_git_tools::diff |
| src/tools/builtin/git/push.rs | 144 | dasclaw_git_tools::push |
| src/tools/builtin/git/status.rs | 143 | dasclaw_git_tools::status |
| src/tools/builtin/git/runner.rs | 112 | dasclaw_git_tools::runner |
| src/tools/builtin/git/mod.rs | 30 | dasclaw_git_tools::lib |
| **小计** | **1,331** | |

#### 1.3.6 搜索工具 → `dasclaw_core::tools::search`

| fork 路径 | LOC | 目标位置 |
|-----------|---:|----------|
| src/tools/builtin/grep_search.rs | 382 | dasclaw_core::tools::search::grep |
| src/tools/builtin/glob_search.rs | 313 | dasclaw_core::tools::search::glob |

#### 1.3.7 Plan / 协作 工具 → `dasclaw_core::tools`

| fork 路径 | LOC | 目标位置 |
|-----------|---:|----------|
| src/tools/builtin/plan_mode.rs | 269 | dasclaw_core::tools::plan_mode |
| src/tools/builtin/session_fork.rs | 170 | dasclaw_core::tools::session_fork |
| src/tools/builtin/sub_agent.rs | 381 | dasclaw_core::tools::sub_agent |

#### 1.3.8 Web 工具 → `dasclaw_core::tools::web`

| fork 路径 | LOC | 目标位置 |
|-----------|---:|----------|
| src/tools/builtin/web_fetch.rs | 372 | dasclaw_core::tools::web::fetch |
| src/tools/builtin/web_search.rs | 533 | dasclaw_core::tools::web::search |

#### 1.3.9 Feature flag → `dasclaw_features`

| fork 路径 | LOC | 目标位置 |
|-----------|---:|----------|
| src/tools/feature_flags.rs | 93 | dasclaw_features::flags |

### 1.4 平台 / 路径

| fork 路径 | LOC | 目标位置 | 备注 |
|-----------|---:|----------|------|
| src/workspace_dir.rs | 183 | dasclaw_core::workspace_dir | 工作区目录解析 |
| src/routines/mod.rs | 31 | **dasclaw_routines**（**31 蓝图缺口**） | routines 子系统入口（小，但应建独立 crate 留扩展） |

### 1.5 可观测

| fork 路径 | LOC | 目标位置 |
|-----------|---:|----------|
| src/observability/prompt_cache.rs | 175 | dasclaw_observability::prompt_cache |

### 1.6 数据库迁移

| fork 路径 | LOC | 目标位置 | 备注 |
|-----------|---:|----------|------|
| migrations/V15__conversation_message_attachments.sql | 1 | admin-backend/migrations/ + desktop-client | 消息附件表（依赖方迁） |

### 1.7 测试（8 个 / 4,936 LOC）

| fork 路径 | LOC | 目标位置 | 备注 |
|-----------|---:|----------|------|
| tests/parity_harness.rs | 2,151 | 顶层 tests/parity_harness.rs | Claude Code 等价性测试主框架 |
| tests/parity_gate_p1.rs | 823 | 顶层 tests/parity_gate_p1.rs | P1 等价性门 |
| tests/parity_gate_p2.rs | 577 | 顶层 tests/parity_gate_p2.rs | P2 等价性门 |
| tests/claw_code_real_llm_tests.rs | 401 | dasclaw_core/tests/ | 真实 LLM 调用测试 |
| tests/agent_hooks_integration_test.rs | 303 | dasclaw_safety/tests/ | Hook 集成 |
| crates/ironclaw_safety/tests/agent_hook_integration.rs | 195 | dasclaw_safety/tests/ | 同上 |
| tests/tool_reachability.rs | 202 | 顶层 tests/ | 工具可达性 |
| tests/p2_e2e_tests.rs | 191 | 顶层 tests/ | P2 E2E |
| tests/p2_security_audit_tests.rs | 90 | desktop-client/tests/ 或 dasclaw_safety/tests/ | 安全审计 |
| **小计** | **4,933** | | |

### 1.8 独家文件归属总计

| 目标 dasclaw_* | 文件数 | LOC |
|----------------|---:|---:|
| dasclaw_core | 13 | ≈ 5,260 |
| dasclaw_lsp（**新建议**） | 5 | 1,694 |
| dasclaw_git_tools（**新建议**） | 9 | 1,331 |
| dasclaw_safety（升自 ironclaw_safety） | 4 | 910 |
| dasclaw_bash_validation | 1 | 657 |
| dasclaw_apply_patch | 1 | 438 |
| dasclaw_sandbox::adapters | 1 | 277 |
| dasclaw_identity::adapters | 1 | 205 |
| dasclaw_observability | 1 | 175 |
| dasclaw_features | 1 | 93 |
| dasclaw_routines（**新建议**） | 1 | 31 |
| desktop-client/src/（壳层） | 1 | 125 |
| 顶层 tests/ + 各 crate tests/ | 9 | ≈ 4,933 |
| migrations/ | 1 | 1 |
| **总计** | **49** | **≈ 14,800** |

---

## 2. 修改上游文件清单（93 个）→ patch 待审

> 这些是 fork 改了 0.26 上游文件的部分，需要逐个 `git diff ironclaw-v0.26.0 HEAD -- <file>` 看 fork 加了什么，标记每段 patch：保留 / 已迁出 / 丢弃。

### 2.1 高频 patch 文件（改 ≥ 5 次，需深度 review）

| fork 路径 | patch 次数 | 评估 |
|-----------|---:|------|
| src/agent/dispatcher.rs | 12 | **桥接 v1↔v2 关键**；待 W3 提取核心 patch 进 dasclaw_bridge_lite |
| src/agent/agent_loop.rs | 9 | 部分已迁 x_claw_agent；剩余 patch 进 dasclaw_core |
| Cargo.toml | 8 | 删 rig-core / 加 x_claw_agent path / 各种 dep 调整；W6 收尾时人工裁剪 |
| src/llm/rig_adapter.rs | 7 | rig-core 已废弃；fork 中残留待整体删除 |
| src/llm/mod.rs | 7 | 暴露新 LLM 模块（claw_code_provider, prompt, schema_utils）；进 dasclaw_core::llm |
| src/agent/thread_ops.rs | 7 | thread 操作扩展；进 dasclaw_core |
| src/worker/job.rs | 6 | jobs 系统；保留 fork 或迁 dasclaw_core::worker |
| src/tools/registry.rs | 5 | 新增 fork 工具注册（git/lsp/plan/sub_agent 等）；进 dasclaw_core::tools |
| src/testing/mod.rs | 5 | 暴露 scripted_llm；进 dasclaw_core::testing |

### 2.2 中频 patch 文件（改 3-4 次，常规 review）

| fork 路径 | patch 次数 |
|-----------|---:|
| src/tools/builtin/mod.rs | 4 |
| src/llm/reasoning.rs | 4 |
| tests/support/gateway_workflow_harness.rs | 3 |
| src/worker/container.rs | 3 |
| src/llm/retry.rs | 3 |
| src/config/llm.rs | 3 |
| src/agent/mod.rs | 3 |
| src/agent/agentic_loop.rs | 3 |

### 2.3 低频 patch 文件（改 1-2 次，机械迁移）

略（共 76 个文件）。完整列表用 `git log ironclaw-v0.26.0..HEAD --no-merges --diff-filter=M --name-only --format="" | sort | uniq -c | sort -rn` 重现。

---

## 3. 31 蓝图缺口（v1 收录建议）

依本清单证据，**31 §4 crate 蓝图缺 3 个**：

| crate（建议补） | 来源依据 | LOC 估算 |
|----------------|----------|---:|
| `dasclaw_lsp` | fork 独家 5 个 LSP 文件 | ≈ 1,700 |
| `dasclaw_git_tools` | fork 独家 9 个 git 工具 | ≈ 1,330 |
| `dasclaw_routines` | fork 独家 routines/ 子系统 + 后续会扩展 | ≈ 30（W1） / 后续多 |

**建议**：在 31 v2.4 中把这 3 个 crate 加入 §4.1 / §4.2 / §4.3 列表，并相应在 W1 骨架阶段补建（当前 14 骨架未含）。

---

## 4. 测试迁移地图

| 测试族 | fork 文件 | LOC | 目标位置 |
|--------|-----------|---:|----------|
| Parity Harness（Claude Code 等价性） | parity_harness.rs / parity_gate_p1.rs / parity_gate_p2.rs | 3,551 | 顶层 tests/ |
| Real LLM 调用 | claw_code_real_llm_tests.rs | 401 | dasclaw_core/tests/ |
| Hook 集成 | agent_hooks_integration_test.rs / crates/ironclaw_safety/tests/agent_hook_integration.rs | 498 | dasclaw_safety/tests/ |
| Tool 可达性 | tool_reachability.rs | 202 | 顶层 tests/ |
| P2 E2E | p2_e2e_tests.rs | 191 | 顶层 tests/ |
| P2 安全审计 | p2_security_audit_tests.rs | 90 | desktop-client/tests/ |

---

## 5. 删 fork 前置条件 Checklist（W6+ 阀门）

> 每项必须打勾才允许 `git rm -rf desktop-client/ironclaw`。

### 5.1 独家文件迁移（49 项）
- [ ] §1.1 LLM 层 7 个文件全部迁入 dasclaw_core::llm（≈ 2,351 LOC）
- [ ] §1.2 Agent 接线层 4 个文件按归属拆分迁出（832 LOC）
- [ ] §1.3 工具层 22 个文件按归属拆分迁出（5,915 LOC，含 dasclaw_lsp / dasclaw_git_tools 新建）
- [ ] §1.4 平台 / 路径 2 个文件迁出（214 LOC）
- [ ] §1.5 observability/prompt_cache.rs 迁入 dasclaw_observability（175 LOC）
- [ ] §1.6 V15 migration 迁入 admin-backend / desktop-client（1 行）
- [ ] §1.7 测试 9 个文件迁出（4,933 LOC）

### 5.2 上游 patch 审查（93 项）
- [ ] §2.1 高频 9 个文件每段 patch 标记保留 / 已迁 / 丢弃
- [ ] §2.2 中频 8 个文件同上
- [ ] §2.3 低频 76 个文件同上

### 5.3 31 蓝图修订
- [ ] 31 v2.4 补 dasclaw_lsp / dasclaw_git_tools / dasclaw_routines 三 crate 蓝图
- [ ] 32 v2.4 在 W1 骨架追加这 3 个 crate（或单列 W1.5 补建）

### 5.4 编译验证
- [ ] `cargo build --workspace` 无 fork 也通过
- [ ] desktop-client 启动后 chat 流水线不回退
- [ ] desktop-client/tests/tauri_command_contract_tests.rs 全过
- [ ] 顶层 tests/parity_gate_p{1,2}.rs 全过

### 5.5 文档归档
- [ ] desktop-client/ironclaw 子目录 git mv 到 archive/legacy-ironclaw-fork-2026-04
- [ ] 本文档（39）打补丁记录每项打勾时间 + 提交 hash

---

## 6. 与 v2.3 文档矩阵的关系

- 本文档**新增**于 v2.3 阶段，作为 W6+ 删 fork 的具体路线图
- 31 v2.3 §4.5 「desktop-client/ironclaw — W1-W6 保留作为私货来源；W6+ 私货全迁出后才删除」的**具体打勾清单**
- 32 v2.3 W6+ 任务的**前置依赖**

## 7. 变更日志

- **v1.0 (2026-04-26)** — 首发。基于 `git log ironclaw-v0.26.0..HEAD --no-merges` 实测 42 commit / 49 独家文件 / 93 修改文件 / ≈ 14,800 LOC 私货。
