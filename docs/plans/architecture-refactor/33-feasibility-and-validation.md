# 33 — 融合可行性分析 + 验证方案 + 复用现有测试工具

> **v2.0 (2026-04-25)** · 基于 [30-architecture-truth.md](30-architecture-truth.md) v2 + [31-target-architecture.md](31-target-architecture.md) v2 + [32-execution-plan.md](32-execution-plan.md) v2 同步升级。
>
> 回答用户的核心问题：
> "搬迁三库的能力后，是否能达到预想的效果？这样每一项都缝合拼接是否有达到预想的效果？怎么验证？现有的三个库是否在代码中带了相关验证或测试能力是否达标的工具，是否能复用？"
>
> **v1 → v2 变更**：
> 1. 预期效果目标 G 从 14 个扩充到 **18 个**（+G15 项目文档 / +G16 PTY / +G17 panic hook / +G18 AuthToken refresh）
> 2. 拼接风险 R 从 7 个扩充到 **10 个**（+R8 项目文档 × hook 顺序 / +R9 IPC 命令表 × 前后端同步 / +R10 双身份（device-key + agent-identity）与 auth 冲突）
> 3. 复用资产扩充 codex login crate / network-proxy / project_doc 测试

---

## 1. 预期效果定义（先把"预想"明确）

如果不明确"预想效果"，就无从验证。基于 27 文档"架构优先 + 客户端基础 + 安全基线"方向，我给出 4 类可验证目标：

### 1.1 架构层目标
- **G1** 边界清晰：每个 dasclaw_* crate 单测可通过，不依赖应用层
- **G2** 唯一 runtime：dasclaw_core 是唯一 agent 引擎，无第二份并存
- **G3** 不可绕过路径：DLP / Sandbox / Hooks 三道防线 100% 经过

### 1.2 能力层目标（来自 30 文档独家能力）
- **G4** 三平台 sandbox 工作：Linux landlock / macOS seatbelt / Win JobObject 各能拦截至少 1 个越权场景
- **G5** Apply Patch 协议工作：lark 语法的 patch 能正确 apply
- **G6** Hooks 引擎结构化：BeforeInbound 拒绝 → 整个 loop 终止
- **G7** governance 6 件套可启用：通过 feature flag 开关，开关后行为一致
- **G8** MCP 6 transport 全可用：stdio/sse/http/ws/sdk/managed-proxy 各跑一个 demo server

### 1.3 客户端基线零回退
- **G9** desktop-client 55 个 Tauri 命令全过契约测试
- **G10** DLP 8 命令零回退（识别率 + 脱敏正确性）
- **G11** Cypress E2E 全部通过

### 1.4 性能 / 体感
- **G12** 推理首字延迟 < 现状 +10%
- **G13** Sandbox 启动开销 < 50ms（参考 codex baseline）
- **G14** 内存占用 < 现状 +20%

### 1.5 v2 新增补齐目标
- **G15** 项目文档加载正确（ADR-106）：AGENTS.md > CLAUDE.md > .codex/agents.md 优先级升限于 `project_doc_max_bytes`，多层合并顺序与 codex/claw 生态一致
- **G16** PTY 体业务接近 codex：resize/信号转发/长会话保持在 macOS+Linux 都能走通
- **G17** panic hook 生效（ADR-109）：人为 panic 产出本地 dump + admin-backend `/api/client-reports` 上报，**不静默丢失**
- **G18** AuthToken refresh 代理位（ADR-107）：到期前 5 min 能自动刷新；连续失败 N 次提示重新登录，不静默 401

---

## 2. "缝合拼接"风险分析

用户担心"每一项都缝合拼接是否有达到预想的效果"。这是合理担忧。我列出**真实风险**和**对应缓解**：

### 2.1 真实存在的拼接风险

| 风险 | 严重性 | 触发条件 | 缓解 |
|------|-------|---------|------|
| **R1：Hook 调用顺序错乱** | 🔴 | codex hooks 引擎 + ironclaw 6 lifecycle + claw-code PreToolUse 三套调用约定不一致 | W3 写**统一 hook 调用契约文档**，所有调用方按统一顺序触发 |
| **R2：Sandbox trait 不能覆盖 Docker** | 🔴 | codex sandbox 是进程级，ironclaw 是容器级，抽象层泄漏 | trait 设计成 `enum SandboxKind { Process(P), Container(C) }`，工具调用方用 enum，不假设 |
| **R3：Governance 决策 vs Hook 决策冲突** | 🟡 | policy_engine 说 escalate，hook 说 reject | 显式优先级：Hook reject > Governance escalate（hook 层在外） |
| **R4：MCP 6 transport 兼容性差** | 🟡 | claw-code 的 ManagedProxy 与 codex 的 OAuth 凭据流不一致 | W5 端到端测试每个 transport，不达标的标 P2 推迟 |
| **R5：dasclaw_core 合并后 v2 CodeAct 引擎丢失** | 🔴 | ironclaw_engine 的 CodeAct 模式 + x_claw_agent 的 agentic_loop 二者协议不一样 | W1 决策点 D1：以 ironclaw_engine 为基础，吸收 hooks |
| **R6：DLP 与 Hook BeforeInbound 重复处理** | 🟢 | 两者都在入口扫描 | 明确分工：DLP 做 PII，Hook 做策略；DLP 在 IPC 层，Hook 在 runtime 层 |
| **R7：性能回退** | 🟡 | 多层防线累积开销 | W8 基线对比，每层 < 5ms 预算 |
| **R8：ProjectDoc 加载 × hook 顺序** | 🟡 | AGENTS.md 载入 × OnSessionStart hook 顺序不一致导致项目上下文丢失 | W3 推动：ProjectDocLoader::load 必须在 OnSessionStart hook 之前运行；加 trace 验证 |
| **R9：IPC 命令表不同步** | 🔴 | LSP/MCP IPC 新增后前后端任一一侧未同步 → React UI 报 "command not found" | 契约测试 FRONTEND_INVOKED_COMMANDS × REGISTERED_COMMANDS 集合相等（AGENTS.md 已明确） |
| **R10：device-key + agent-identity 与 auth_token 冲突** | 🟡 | desktop 用 keychain，codex device-key 用 P256，两者身份优先级不明 | W7 在 ADR 中定：device-key 用于设备识别，auth_token 用于会话，不互换 |

### 2.2 拼接成功的判据

如果**下面 5 条都满足**，可以认为"缝合"成功：

1. ✅ dasclaw_core 单元测试覆盖率 > 90%，不依赖任何 desktop-client 代码即可跑通
2. ✅ desktop-client 重构后所有现有契约测试 + cypress 测试通过
3. ✅ 三平台 sandbox 在 GitHub Actions matrix 全部绿
4. ✅ 关闭 governance feature flag 后行为与 W1 基线一致（说明可选模块真的可选）
5. ✅ 性能基准回退 < 10%

### 2.3 拼接失败的早期信号

监控以下指标，触发即停下评估：

- 新 crate 之间出现循环依赖
- trait 接口频繁修改（>2 次/周）
- 测试覆盖率上不去（<70%）
- desktop-client 启动时间增加 > 50%
- Hook 链路 trace 出现 race condition

---

## 3. 验证方案：分层验证矩阵

按 AGENTS.md "六维测试矩阵"组织：

### 3.1 单元测试（每个 crate）

| crate | 重点测试 | 必须达标 |
|-------|---------|---------|
| dasclaw_core | agent loop 状态机 / session 隔离 / context 压缩触发 | 覆盖率 90% |
| dasclaw_sandbox | 三平台 fail-safe（沙箱不可用时拒绝） | 失败路径 100% |
| dasclaw_hooks | 6 lifecycle × 3 决策（pass/modify/reject） | 18 组合全过 |
| dasclaw_apply_patch | lark 语法 30+ 用例 | 100%（直接复用 codex 测试） |
| dasclaw_governance | 18 unit + 6 integration（claw-code 现成） | 100% |
| dasclaw_mcp | 6 transport × connect/list_tools/invoke | 18 组合全过 |
| dasclaw_execpolicy | Starlark 规则解析 | 90% |

### 3.2 契约测试

- **Tauri IPC 契约**（已存在）：desktop-client/tests/tauri_command_contract_tests.rs
- **新增**：每个 dasclaw_* trait 加契约测试，验证多实现行为一致
- **新增**：DLP 业务契约（敏感词识别率 > 95%）

### 3.3 失败路径测试（fail-safe 优先）

| 路径 | 触发故障 | 预期行为 |
|------|---------|---------|
| Sandbox 不可用 | landlock kernel 未启用 | 拒绝执行（不降级为直接 exec） |
| Hook 引擎 panic | Hook 实现内部 panic | 中断 loop，不静默忽略；panic hook（dasclaw_crash）同时产出 dump |
| DLP 字典加载失败 | 字典文件损坏 | 拒绝消息（不放行） |
| LLM provider 超时 | reqwest timeout | failover 到下家，全失败时拒绝 |
| MCP server 断开 | TCP RST | 标记 unhealthy，重试 N 次，超限拒绝调用 |
| **AGENTS.md 加载超时**（v2 新增）| 项目文档过大/磁盘卡顿 | 2s 超时后降级为空文档 + WARN log，不阻塞启动 |
| **panic hook 未安装**（v2 新增）| `dasclaw_crash::install_panic_hook()` 未被 main 调用 | 启动时序测试必需验证 panic hook 已安装 |
| **AuthToken refresh 连续失败**（v2 新增）| OAuth 服务器 5xx | 连续 N 次 → emit `auth-expired` 事件让前端引导重新登录，不静默 401 |
| **Approval WebSocket 断开**（v2 新增）| 网络闪断 | 自动降级为 30s polling，恢复后重连 WebSocket | 

### 3.4 集成测试

- **沙箱集成**：构造 1 个尝试越权的工具脚本，验证三平台都拦截
- **完整链路**：UI → IPC → DLP → Core → Hook → Sandbox → Tool → LLM → 响应链路单端到端 trace
- **Routines 集成**：Cron 触发 → agent 执行 → 经过完整防线

### 3.5 安全审计测试

- 日志中不含原始 PII / API key（grep 测试）
- 错误响应中不含敏感字段
- Hook 拒绝时不泄漏内部决策原因到 UI 层（仅 trace 留存）

### 3.6 PICT 组合测试（参考 AGENTS.md PICT 章节）

针对**多参数高风险**场景生成 pairwise 矩阵：

```
# pict 模型示例：sandbox × dlp × hook × governance
Platform: linux, macos, windows
SandboxBackend: process, docker, none
DlpEnabled: true, false
HookDecision: pass, modify, reject
GovernanceEnabled: true, false
ToolCategory: file, shell, http, mcp

# 约束
IF [SandboxBackend] = "none" THEN [HookDecision] = "reject";  # 没沙箱必须 hook 拒绝
IF [DlpEnabled] = false AND [Platform] = "windows" THEN [GovernanceEnabled] = true;  # 政企默认开
```

执行：`python3 scripts/pict_generate.py docs/plans/architecture-refactor/sandbox-matrix.pict`

### 3.7 E2E（Cypress + Playwright）

- desktop-client/cypress 现有用例全部通过
- 新增：Plan Mode + Approval 流程 E2E
- 新增：Routine 触发 → Hook 拦截 → 用户审批 全流程

---

## 4. 复用三库现有测试工具（关键问题）

回答"现有三个库是否带验证/测试工具，能否复用"——**答案：能复用约 60-70%**。

### 4.1 codex-cli-main 可复用资产

| 工具 / 测试 | 路径 | 复用方式 |
|-----------|------|---------|
| **apply-patch fixture** | codex-cli-main/codex-rs/apply-patch/tests/fixtures/ | 直接 copy 进 dasclaw_apply_patch/tests/ |
| **sandbox 三平台测试** | codex-cli-main/codex-rs/linux-sandbox/tests/ + sandboxing/tests/ | 直接 port 到 dasclaw_sandbox |
| **hooks 引擎测试** | codex-cli-main/codex-rs/hooks/tests/ + core/tests/hook_*.rs | 直接 port 到 dasclaw_hooks |
| **execpolicy 测试** | codex-cli-main/codex-rs/execpolicy/tests/ + Starlark 规则集 | 直接 port |
| **rollout-trace 工具** | codex-cli-main/codex-rs/rollout-trace/ | 作为 dasclaw_observability 的实现 |
| **CI matrix 模板** | codex-cli-main/.github/workflows/ | 参考三平台 CI 配置 |
| **codex `analytics` CompactionEvent** | codex-cli-main/codex-rs/analytics/src/facts.rs | 作为 dasclaw_observability 的事件 schema 基线 |
| **prompt_debug** | codex-cli-main/codex-rs/prompt_debug/ | 调试工具，不进生产但保留作 dev 工具 |
| **codex_thread test infrastructure** | codex-cli-main/codex-rs/core/src/codex_thread.rs | session 测试参考 |
| **core_test_support / mcp_test_support**（v2 新增）| codex-cli-main/codex-rs/core/src/test_support/ + mcp-server/test_support/ | 成熟的 MCP server mock + LLM provider mock，直接用于 dasclaw_mcp/dasclaw_llm 单元 |
| **codex login crate 测试**（v2 新增）| codex-cli-main/codex-rs/login/tests/ | OAuth refresh 流程与 P256 device-key 测试，支撑 G18 |
| **network-proxy** crate（v2 新增）| codex-cli-main/codex-rs/network-proxy/ | 企业代理 / mTLS 测试资产 |
| **project_doc 加载测试**（v2 新增）| codex-cli-main/codex-rs/core/src/project_doc.rs 内含的 #[cfg(test)] | AGENTS.md 多层合并 + project_doc_max_bytes 限制测试，支撑 G15 |
| **portable-pty cross-platform tests**（v2 新增）| codex-cli-main/codex-rs/exec/src/exec_command/ | resize / signal forward / session lifecycle，支撑 G16 |

**估算**：直接 port + 适配 = **~5-8 周节省**（不用从零写沙箱测试）

### 4.2 claw-code 可复用资产

| 工具 / 测试 | 路径 | 复用方式 |
|-----------|------|---------|
| **policy_engine 测试** | claw-code/rust/crates/runtime/tests/policy_*.rs | 直接 port，18 unit + 6 integration |
| **recovery_recipes 测试** | claw-code/rust/crates/runtime/tests/recovery_*.rs | 直接 port，15 unit + 1 integration |
| **branch_lock 测试** | claw-code/rust/crates/runtime/tests/branch_lock_*.rs | 直接 port |
| **stale_branch 策略测试** | claw-code/rust/crates/runtime/tests/stale_*.rs | 直接 port |
| **green_contract 测试** | claw-code/rust/crates/runtime/tests/green_*.rs | 直接 port |
| **trust_resolver 测试** | claw-code/rust/crates/runtime/tests/trust_*.rs | 直接 port |
| **mcp 6 transport 集成测试** | claw-code/rust/crates/runtime/tests/mcp_*.rs | 直接 port |
| **mock-anthropic-service** | claw-code/rust/crates/mock-anthropic-service/ | **作为开发测试 mock**（不进生产，但 dev 阶段可用） |
| **compat-harness** | claw-code/rust/crates/compat-harness/ | **作为 LLM 兼容性测试**（dev 工具） |
| **bash_validation 模块** | claw-code/rust/crates/runtime/src/bash_validation.rs（1004 LOC，6 验证模块） | 内含丰富 bash 注入检测，作为 dasclaw_safety 增强 |
| **mock-anthropic 27 场景**（v2 新增）| claw-code/rust/crates/mock-anthropic-service/scenarios/ | 预烘 27 个 Anthropic API 场景（streaming/tool_use/error），直接用于 dasclaw_llm + agent loop 契约测试 |

**估算**：直接 port + 适配 = **~3-4 周节省**

### 4.3 ironclaw-main 可复用资产

| 工具 / 测试 | 路径 | 复用方式 |
|-----------|------|---------|
| **engine_startup_tests 模式** | desktop-client/src/engine_startup_tests.rs（已存在） | 启动时序测试模式 |
| **integration_smoke_tests 模式** | admin-backend/tests/integration_smoke_tests.rs（已有） | 三层防护：编译+迁移+路由 |
| **tauri_command_contract_tests** | desktop-client/tests/tauri_command_contract_tests.rs | IPC 契约保护 |
| **ironclaw safety 测试** | desktop-client/ironclaw/crates/ironclaw_safety/tests/ | 移到 dasclaw_safety 后保留 |
| **observability 后端** | ironclaw-main/src/observability/ | 三种后端（noop/log/multi）可直接用 |
| **failover / circuit_breaker 测试** | ironclaw-main/src/llm/failover_tests.rs | LLM 弹性测试参考 |
| **DLP property tests** | desktop-client/tests/policy_sync_property_tests.rs | property-based 模式参考 |
| **cypress E2E 用例** | desktop-client/cypress/ | 全保留 |
| **HTTP recording/replay**（v2 新增）| ironclaw-main/src/llm/recording.rs | LLM 交互记录为 JSON，重放于集成测试，避免访问真实 API；支撑 G12/G18 |
| **`device-key` + `agent-identity` keychain 测试**（v2 新增）| desktop-client/src/identity/ 现有 | 复用身份物流测试，避免 R10 冲突 |

**估算**：保留为主 = **~2 周节省**（不用重写）

### 4.4 不可复用 / 需要重写

| 内容 | 原因 |
|------|------|
| codex/tui 测试 | TUI 不搬，测试无意义 |
| codex/app-server 测试 | 不搬 |
| claw rusty-claude-cli 测试 | CLI 不搬 |
| ironclaw_tui 测试 | TUI 不搬 |
| ironclaw_gateway 完整 UI 测试 | 桌面端用 React UI，部分保留 |

---

## 5. 验证工具链整合

### 5.1 已有工具（继续用）
- `cargo test` / `cargo build` / `cargo clippy`
- `cargo-llvm-cov`（覆盖率）
- `scripts/coverage.sh`（项目内）
- `scripts/pict_generate.py`（PICT 组合测试）
- `cypress`（E2E）
- `proptest`（property-based）
- `wiremock` / `mockito`（HTTP mock）

### 5.2 新增工具（按需）
- **codex 的 prompt_debug**（dev 工具，调试 LLM 推理）
- **claw-code 的 mock-anthropic-service**（LLM mock 测试）
- **codex 的 analytics CompactionEvent**（事件 schema 标准）

### 5.3 CI 流水线增强

```yaml
# .github/workflows/ci.yml 关键 job
jobs:
  build-test:
    strategy:
      matrix:
        os: [ubuntu-22.04, macos-14, windows-2022]
    steps:
      - cargo build --workspace
      - cargo test --workspace
      - cargo clippy --workspace -- -D warnings

  contract-test:
    steps:
      - cargo test --test tauri_command_contract_tests
      - cargo test --test integration_smoke_tests

  coverage:
    steps:
      - ./scripts/coverage.sh

  e2e:
    steps:
      - cd desktop-client/cypress && npm run e2e

  pict:
    steps:
      - python3 scripts/pict_generate.py docs/plans/architecture-refactor/*.pict
```

---

## 6. 最终结论：能否达到预想效果

### ✅ 可以达到的部分

| 目标 | 信心度 | 依据 |
|------|-------|------|
| G1 边界清晰 | 🟢 高 | 31 文档已设计层次约束，crate 目录隔离 |
| G2 唯一 runtime | 🟢 高 | W1 删 fork 后强制 |
| G3 三道防线不可绕过 | 🟢 高 | 在 trait 层强制（NoopSandbox 也是经过路径） |
| G4 三平台 sandbox | 🟢 高 | codex 已生产验证，直接 port |
| G5 Apply Patch | 🟢 高 | codex 已有完整 lark 实现 |
| G6 Hooks 结构化 | 🟢 高 | codex hooks crate 完成度高 |
| G9 客户端 IPC 不回退 | 🟢 高 | 契约测试守门 |
| G10 DLP 不回退 | 🟢 高 | 不动 DLP 模块，只换 runtime 桩 |
| G15 项目文档加载（v2）| 🟢 高 | 直接 port codex `project_doc.rs` |
| G17 panic hook（v2）| 🟢 高 | 标准 Rust API，添加 `dasclaw_crash::install()` 即可 |

### ⚠️ 有风险的部分

| 目标 | 信心度 | 风险 |
|------|-------|------|
| G7 governance 可启用 | 🟡 中 | claw-code 6 件套与新 hooks/sandbox 集成复杂度未知 |
| G8 MCP 6 transport | 🟡 中 | claw-code 部分 transport 实现可能未在生产环境验证 |
| G12 推理延迟 +10% | 🟡 中 | 多层防线累积，需 W8 实测 |
| G13 Sandbox <50ms | 🟡 中 | 取决于平台，Windows JobObject 启动开销可能更大 |
| G14 内存 +20% | 🟡 中 | 多 crate 开销 + 重复结构体可能超预算 |
| G16 PTY 跨平台（v2）| 🟡 中 | portable-pty 跨平台细节差异，Windows ConPTY 需额外测试 |
| G18 AuthToken refresh（v2）| 🟡 中 | OAuth 服务端不可控，需可靠的失败路径 + WebSocket 推送通知 |

### ❌ 可能达不到的部分

无明确"达不到"的目标，但**进度风险高**：6-8 个月时间线**取决于团队规模**。如果只有 1-2 人专职，更现实是 8-10 个月。

---

## 7. 给用户的最终建议

### 7.1 在动手 W1 之前必须做的 3 件事
1. **确认 31 文档 §8 D1-D6 决策点**（特别是 D1 唯一 runtime 选择）
2. **明确团队规模和并行度**（影响 W2/W3 是否真的能并行）
3. **签字 ADR-104 non-goal 清单**（避免后续重新讨论）

### 7.2 W1 启动 PoC（建议 1 周内完成）
做一个**最小可行原型**验证拼接能否真的通：
- 删 desktop-client/ironclaw fork（1 天）
- 把 ironclaw-main 0.26 接到顶层 workspace（1 天）
- 跑 desktop-client，确保 chat 流水线不崩（2 天）
- 起 dasclaw_core 空 crate，定义 1 个 trait（1 天）

如果**这一周都做不下来**，说明融合复杂度被低估，需要重新评估。

### 7.3 持续监控指标
W1-W9 每周看：
- `cargo build --workspace` 时长
- 测试覆盖率
- desktop-client 启动时间
- 推理首字延迟基线
- Hook 链路 trace 完整性

任意指标连续 2 周恶化 > 20% 触发架构 review。

---

## 附录 A — 相关测试工具复用清单（可执行）

```bash
# 1. 从 codex 复用 sandbox 测试（W2）
cp -r codex-cli-main/codex-rs/linux-sandbox/tests crates/dasclaw_sandbox/tests/linux/
cp -r codex-cli-main/codex-rs/sandboxing/tests crates/dasclaw_sandbox/tests/macos/

# 2. 从 codex 复用 apply-patch fixture（W3）
cp -r codex-cli-main/codex-rs/apply-patch/tests/fixtures crates/dasclaw_apply_patch/tests/

# 3. 从 codex 复用 hooks 测试（W3）
cp -r codex-cli-main/codex-rs/hooks/tests crates/dasclaw_hooks/tests/

# 4. 从 claw-code 复用 governance 测试（W4）
cp claw-code/rust/crates/runtime/tests/policy_*.rs crates/dasclaw_governance/tests/
cp claw-code/rust/crates/runtime/tests/recovery_*.rs crates/dasclaw_governance/tests/
cp claw-code/rust/crates/runtime/tests/branch_*.rs crates/dasclaw_governance/tests/
cp claw-code/rust/crates/runtime/tests/stale_*.rs crates/dasclaw_governance/tests/
cp claw-code/rust/crates/runtime/tests/green_*.rs crates/dasclaw_governance/tests/
cp claw-code/rust/crates/runtime/tests/trust_*.rs crates/dasclaw_governance/tests/

# 5. 从 claw-code 复用 mock-anthropic-service（dev tool，W3 起）
cp -r claw-code/rust/crates/mock-anthropic-service crates/dasclaw_dev_mock/
```
