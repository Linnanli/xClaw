# 17 — 三库 Crate 清单盘点与归属审计

> **用途**: 给 [11-target-architecture-route-b.md §8](11-target-architecture-route-b.md) 的 Wave 路线图提供**落地依据**。按三库真实 crate 目录盘点,为每个 crate 标记**保留 / port / 废弃 / adapter / 新建** 五态归属,确保无孤儿 crate。
>
> **基线**: 三库总计 **~109 个 crate** (codex 78 + claw-code 9 + ironclaw 19 + shared 3)。本文档不逐一罗列 LOC,只做归属分类。精确 LOC 由 `tokei` 在各 Wave 启动时按需统计。

---

## 0. 盘点方法论

### 0.1 归属状态定义

| 状态 | 含义 | 典型处置 |
|-----|-----|--------|
| ✅ **保留** | 继续用 ironclaw 原版,不动 | 无 port,Cargo 依赖保持 |
| 🔄 **port** | 从 codex/claw-code 迁到 `dasclaw_*` | 新 crate,逐步切换 |
| 🔌 **adapter** | 保留接口,内部换成 port 过来的新 crate | trait 适配层 |
| ❌ **废弃** | Route B 不再需要 | 下线,从 Cargo 剔除 |
| ⭐ **新建** | 三库都没有,由产品北极星 v1.1 驱动新增 | 全新 crate |

### 0.2 盘点工具链 (未执行,仅登记可复用指令)

```bash
# LOC 统计
tokei codex-cli-main/codex-rs claw-code/rust/crates desktop-client/ironclaw

# 模块依赖
cargo modules structure -p <crate>

# 反向引用检查
cargo tree -p <crate> -i

# crate 间环检测
cargo-deps-check  # (需安装)
```

> 注: 17 文档不内嵌 LOC 数字,避免和 13/14 出现三处不一致的维护负担。LOC 唯一权威在 13 文档。

---

## 1. Codex-rs 盘点 (78 个 crate)

codex-rs 是 Route B 的**主要 port 源**,按职能分 7 组:

### 1.1 🔄 Agent 内核 — W1-W4 **Port 核心**

| crate | Wave | 目标命名 |
|-------|-----|--------|
| `core` | W1 | `dasclaw_agent_kernel` (主 port 目标) |
| `protocol` / `protocol-ts` | W1 | 与 `dasclaw_agent_kernel` 合并 |
| `config` | W1 | `dasclaw_agent_kernel::config` |
| `model-provider` / `model-provider-info` | W1 | 同上 |
| `models-manager` | W1 | 同上 |

### 1.2 🔄 Context & Tasks — W3-W4

| crate | Wave | 目标 |
|-------|-----|-----|
| `core` 中 `context_manager/` 模块 | W4 | `dasclaw_context_mgr` (含 snip/micro/budget/threshold) |
| `core` 中 `tasks/` 模块 | W3 | `dasclaw_tasks` (含 ghost_snapshot) |
| `core` 中 `session/` 模块 | W2 | `dasclaw_session` |

### 1.3 🔄 Sandbox 三平台 — W7 (P0 in [14 §2.1](14-claude-code-capability-parity.md))

| crate | Wave | 目标 |
|-------|-----|-----|
| `linux-sandbox` | W7 | `dasclaw_sandbox_linux` (~4780 行) |
| `sandboxing` (macOS seatbelt) | W7 | `dasclaw_sandbox_macos` (~721 行 + .sbpl) |
| (Windows `windows-sandbox-rs` 源) | W7 | `dasclaw_sandbox_windows` (~9753 行) |
| `process-hardening` | W7 | 并入 `dasclaw_sandbox` 管理器 |
| `network-proxy` | W7 | 并入 `dasclaw_sandbox::proxy_routing` |
| `execpolicy` / `execpolicy-legacy` | W7 | `dasclaw_sandbox::policy` |

### 1.4 🔄 支撑层 — W5/W6/W9 并行

| crate | Wave | 目标 |
|-------|-----|-----|
| `apply-patch` | W5 | `dasclaw_apply_patch` |
| `git-utils` | W5 | `dasclaw_git_utils` |
| `hooks` | W6 | `dasclaw_hooks_engine` |
| `features` | W6 | `dasclaw_features` |
| `rollout-trace` | W9 | `dasclaw_rollout_trace` |
| `agent-identity` | W9 | `dasclaw_device_identity` |
| `core-skills` / `skills` | W6 | `dasclaw_skills` (叠加 ironclaw `attenuation.rs` + `gating.rs`, 见 [13 §5.6.1](13-security-capability-inventory.md)) |
| `mcp-server` / `rmcp-client` / `codex-mcp` | W6 | 并入 `dasclaw_mcp` |

### 1.5 ❌ Route B 不采纳 (永久 Non-goal)

| crate | 原因 (15 §5) |
|-------|------------|
| `app-server` / `app-server-*` | Route B 无"app server"概念,走 desktop-client + admin-backend |
| `cloud-tasks` / `cloud-tasks-*` | 永久不做 SaaS/公有云 |
| `chatgpt` / `responses-api-proxy` | 只做 OpenAI 一家的 proxy,国产化不需要 |
| `aws-auth` | 国央企无 AWS |
| `realtime-webrtc` | MVP 非实时语音场景 |
| `backend-client` / `debug-client` | codex 专属 OpenAI backend |
| `collaboration-mode-templates` | 不做多人协作 IDE 场景 |
| `codex-backend-openapi-models` | 同 backend-client |

### 1.6 ⚠️ 待决策 (需产品确认)

| crate | 问题 |
|-------|-----|
| `otel` | 国央企是否允许 OTel 协议出境? Y1 Q1 确认, 倾向 port 但默认关 |
| `telemetry` (if exists) | 同上 |
| `login` / `keyring-store` / `device-key` | 与 `dasclaw_secure_store` (国密) 的关系 — 倾向: login → 适配国密 CA, keyring → 国密卡适配器 |
| `cli` / `exec` / `arg0` / `analytics` | 命令行入口 — desktop-client 是主客户端, Y1 是否同时维护 CLI 待定 |
| `feedback` / `file-search` | 功能小工具 — 按需 port |
| `connectors` | 外部服务桥接 — 看是否覆盖 ironclaw tools-src 已有能力 |

### 1.7 🔌 轻量 adapter

| crate | Wave | 说明 |
|-------|-----|-----|
| `ansi-escape` / `async-utils` / `plugin` / `core-plugins` | 随 W1 进来 | codex 内部工具,跟着 `core` 一起 port |
| `install-context` | W9 | 设备首装上下文, 配合 device_identity |

---

## 2. Claw-code/rust 盘点 (9 个 crate)

```
claw-code/rust/crates/
├── api/                  ← SDK 对外接口
├── commands/             ← /init /ide /skills 等斜杠指令
├── compat-harness/       ← 兼容性测试辅助
├── mock-anthropic-service/ ← 测试 mock
├── plugins/              ← 插件系统
├── runtime/              ← 主执行运行时 (含 prompt/mcp_client/tools/context)
├── rusty-claude-cli/     ← CLI 入口
├── telemetry/            ← 遥测 (脱敏)
└── tools/                ← 工具实现
```

### 归属

| crate | 状态 | Wave | 去处 |
|-------|-----|-----|-----|
| `runtime` | 🔄 **主 port 源之一** | W1-W4 | 拆分进 `dasclaw_agent_kernel` / `dasclaw_context_mgr` / `dasclaw_session`; 系统提示 `prompt.rs:480` 作为 PI 基线 |
| `commands` | 🔄 | W6 | port `/ide` / `/init` / `/skills` 到 `dasclaw_commands` (与 codex cli 差异对齐) |
| `tools` | 🔌 | W1 | 工具实现基线,最终由 `dasclaw_agent_kernel` 的 tool trait 统一 |
| `mcp_client` (in runtime) | 🔄 | W6 | 6 种 transport (stdio/sse/http/ws/sdk/managed-proxy) 比 codex 多,以此为 `dasclaw_mcp` 基线 (见 [14 §4.1 #6](14-claude-code-capability-parity.md)) |
| `plugins` | 🔄 | W6 | port 进 `dasclaw_plugins` |
| `api` | 🔌 | W1 | 作为 `dasclaw_agent_kernel` 的对外 SDK 层 |
| `telemetry` | 🔌 | W9 | 并入 `dasclaw_rollout_trace` 或独立 `dasclaw_telemetry` |
| `compat-harness` | ❌ | — | 本项目不需要兼容性 harness |
| `mock-anthropic-service` | 🔌 | 测试层 | 保留作测试工具,不进生产 |
| `rusty-claude-cli` | ⚠️ | — | 与 codex `cli` 的取舍同时决定 |

---

## 3. Ironclaw 盘点 (19+ 个 crate)

### 3.1 ✅ 全部保留 (Route B 核心 moat)

| crate | 状态 | 理由 |
|-------|-----|-----|
| `desktop-client/ironclaw/crates/ironclaw_common` | ✅ 保留 | 跨模块基础类型 |
| `desktop-client/ironclaw/crates/ironclaw_safety` | ✅ **保留不动** | 4849 行 + fuzz,[13 §2.6](13-security-capability-inventory.md) 项目最高质量安全代码 |
| `crates/ironclaw_auth` (shared) | ✅ 保留 | admin + desktop + scanner 共享 |
| `crates/ironclaw_workspace_cap` (shared) | ✅ 保留 | 568 行 cap_std TOCTOU-safe, [13 §2.8](13-security-capability-inventory.md) |
| `crates/x_claw_agent` (shared) | 🔌 adapter | Hook trait 在 W1 迁到 `dasclaw_agent_kernel`, 此 crate 变薄 |

### 3.2 ironclaw 主工程 `src/` 内的模块 (非独立 crate 但关键)

| 模块 | 状态 | 理由 | 见 |
|------|-----|-----|-----|
| `tools/wasm/` (15 文件) | ✅ 保留 | L4 WASM capability opt-in | [13 §2.2](13-security-capability-inventory.md) |
| `secrets/` (2546 行) | ✅ 保留 + ⭐扩展 | OS Keychain 保留, 叠加 `dasclaw_secure_store` 国密 | [13 §2.3](13-security-capability-inventory.md) |
| `sandbox/` + `sandbox/proxy/` | ✅ 保留 | L3 Docker host 侧, 3611 行 | [13 §2.4/2.5](13-security-capability-inventory.md) |
| `worker/` (5343 行) | ✅ 保留 | L3 容器 guest runtime + ProxyLlmProvider | [13 §5.6](13-security-capability-inventory.md) |
| `extensions/` (11786 行) | ✅ 保留 | Extensions 沙箱 | [13 §2.1](13-security-capability-inventory.md) |
| `skills/` (3665 行) | 🔌 adapter | 骨架 port codex, 保留 `attenuation.rs` + `gating.rs` | [13 §5.6.1](13-security-capability-inventory.md) |
| `tools/redaction.rs` | ✅ 保留 | 18+ 字段脱敏 | [13 §2.7](13-security-capability-inventory.md) |
| `compaction.rs` / `context_monitor.rs` / `session.rs` / `session_manager.rs` | ❌ 下线 | 被 W2/W4 替代 | [11 §8](11-target-architecture-route-b.md) |
| `dispatcher/` / `router/` / `thread_ops/` | ✅ 保留 | 胶水层,连接 kernel 与 skills/jobs/channels | [11 §9](11-target-architecture-route-b.md) |
| `evaluation/` (965 行) | ✅ 保留 | B 端产品差异能力 | [13 §5.6](13-security-capability-inventory.md) |
| `observability/` (835 行) | ✅ 保留 | Trait-based Observer 插件 | [13 §5.6](13-security-capability-inventory.md) |

### 3.3 Channels / Tools-src (12 tools + 5 channels)

| 类别 | 状态 | 说明 |
|------|-----|-----|
| `channels-src/{discord,feishu,slack,telegram,whatsapp}` | ⚠️ 按客户 | MVP 默认只保留 **企微/飞书** 相关, 其他按客户订单启用 |
| `tools-src/{github,gmail,google-*}` | ⚠️ 按客户 | 国央企不用 Gmail/Google,MVP 默认关 |
| `tools-src/{slack,telegram}` | ⚠️ 按客户 | 同 channels |
| `tools-src/web-search` | ✅ 保留 | 改走国产搜索引擎 |
| `tools-src/llm-context` | ✅ 保留 | 通用能力 |

**产品策略**: 海外 channels/tools **不删除代码**, 通过 Feature Flag (W6 `dasclaw_feature_flags`) 默认关闭, 按客户订单开启。

---

## 4. 共享 Crate `crates/` 盘点

| crate | 状态 | Wave | 说明 |
|-------|-----|-----|-----|
| `ironclaw_auth` | ✅ 保留 | — | 多项目共享鉴权 |
| `ironclaw_workspace_cap` | ✅ 保留 | — | L1 cap_std |
| `x_claw_agent` | 🔌 adapter | W1 | Hook trait 迁出后变薄 |

---

## 5. ⭐ 新建 Crate (产品北极星 v1.1 驱动, 三库无源)

| 新 crate | Wave | 依据 | 来源 |
|---------|-----|-----|-----|
| `dasclaw_agent_kernel` | W1 | port codex `core/agent` | 🔄 port |
| `dasclaw_session` | W2 | port codex `core/session` | 🔄 port |
| `dasclaw_tasks` | W3 | port codex `core/tasks` | 🔄 port |
| `dasclaw_context_mgr` | W4 | port codex `core/context_manager` + claw-code `prompt.rs` | 🔄 port |
| `dasclaw_apply_patch` | W5 | port codex `apply-patch` | 🔄 port |
| `dasclaw_git_utils` | W5 | port codex `git-utils` | 🔄 port |
| `dasclaw_hooks_engine` | W6 | port codex `hooks` | 🔄 port |
| `dasclaw_features` | W6 | port codex `features` | 🔄 port |
| `dasclaw_feature_flags` | W6 | 独立 | [14 §3](14-claude-code-capability-parity.md) W5 新建 |
| `dasclaw_sandbox` | W7 | port codex `sandboxing` 管理器 | 🔄 port |
| `dasclaw_sandbox_linux` | W7 | port codex `linux-sandbox` | 🔄 port |
| `dasclaw_sandbox_windows` | W7 | port codex `windows-sandbox-rs` | 🔄 port |
| `dasclaw_sandbox_macos` | W7 | port codex `sandboxing/seatbelt` | 🔄 port |
| `dasclaw_sandbox_resources` | W7 | cgroups 补位 | ⭐ 独家 ([13 §5.5.2](13-security-capability-inventory.md)) |
| `dasclaw_policy` | W8 | 聚合 claw-code policy_engine 等 | 🔄 port |
| `dasclaw_branch_guard` | W8 | 聚合 claw-code branch_lock 等 | 🔄 port |
| `dasclaw_rollout_trace` | W9 | port codex `rollout-trace` | 🔄 port |
| `dasclaw_device_identity` | W9 | port codex `agent-identity` | 🔄 port |
| `dasclaw_skills` | W6 | port codex skills + 叠加 ironclaw attenuation | 🔄 port + ✅ 合并 |
| `dasclaw_commands` | W6 | port claw-code `commands` | 🔄 port |
| `dasclaw_plugins` | W6 | port claw-code `plugins` | 🔄 port |
| `dasclaw_mcp` | W6 | 基于 claw-code `mcp_client` 6 种 transport | 🔄 port |
| **`dasclaw_bash_guard`** | **W0** | **Claude Code 对标 P0** | ⭐ 独家 ([14 §2.1 #1-#2](14-claude-code-capability-parity.md)) |
| **`dasclaw_secure_store`** | **W0** | **TPM/SE/国密 SM2/SM3/SM4** | ⭐ 独家 ([14 §2.1 #4](14-claude-code-capability-parity.md)) |
| **admin-backend handlers 扩展 (非 crate)** | **W0-W2** | 组织管控/审计/SkillsHub 签名 | ⭐ 独家 ([14 §2.1 #5/#6/#7](14-claude-code-capability-parity.md)) |
| **admin-backend/ui/agents.tsx (非 crate)** | **W_Q3** | 可视化 Agent 编辑器 | ⭐ 独家 ([14 §2.1 #8](14-claude-code-capability-parity.md)) |

**新 crate 总数**: 22 个 Rust crate (19 port + 3 独家新建) + 2 处 admin-backend 内部扩展。

**与之前规划对比**: [11 §7.3](11-target-architecture-route-b.md) 原文说 "14 个新 crate", v1.1 扩展至 **22 个**, 原因是 W7 沙箱从 1 个拆为 5 个 + W0 新增 bash_guard + secure_store + W6 commands/plugins/mcp 独立 + W8 policy/branch_guard 拆分。

---

## 6. 归属一致性检查矩阵

> 对账: [11 §8 Wave 路线图](11-target-architecture-route-b.md) vs 本文档 §5, 每个 Wave 是否都有 crate 归属?

| Wave | 11 文档描述 | 对应 crate (本文档) | 一致性 |
|------|-----------|------------------|-------|
| W0 | 🅰 bash_guard + CVE / 🅱 SkillsHub 签名 + 国密 + Fuzz | `dasclaw_bash_guard` + `dasclaw_secure_store` + admin-backend 扩展 + fuzz target | ✅ |
| W0-W2 | 治理后台重构 | admin-backend handlers (org/quota/policy/audit) | ✅ (非 crate) |
| W1 | Agent Kernel | `dasclaw_agent_kernel` | ✅ |
| W2 | Session 分层 | `dasclaw_session` | ✅ |
| W3 | Tasks 与快照 | `dasclaw_tasks` | ✅ |
| W4 | Context Manager | `dasclaw_context_mgr` | ✅ |
| W5 | Patch 与 Git | `dasclaw_apply_patch` + `dasclaw_git_utils` | ✅ |
| W6 | Hooks/Features/Skills/MCP/Commands/Plugins | 6 个 crate | ✅ |
| W7 | 沙箱平台覆盖 | 5 个 sandbox crate | ✅ |
| W8 | 合规与分支治理 | `dasclaw_policy` + `dasclaw_branch_guard` | ✅ |
| W9 | 可观测与身份 | `dasclaw_rollout_trace` + `dasclaw_device_identity` | ✅ |
| W10 | 集成测试 + sub_agent 重写 | 无新 crate, 改 `ironclaw tools/builtin/sub_agent.rs` | ✅ |
| W_Q3 | 可视化 Agent 编辑器 | admin-backend/ui 扩展 | ✅ |

**结论**: 13 个 Wave 全部有明确 crate 归属, 无孤儿。

---

## 7. 废弃清单 (❌ Route B 不再需要)

| 项 | 原因 |
|---|-----|
| codex `app-server*` / `cloud-tasks*` / `chatgpt` / `responses-api-proxy` | SaaS/公有云 Non-goal |
| codex `aws-auth` / `backend-client` / `realtime-webrtc` / `collaboration-mode-templates` | 国央企场景不适用 |
| claw-code `compat-harness` | 本项目无兼容性测试需求 |
| ironclaw `compaction.rs` / `context_monitor.rs` / `session.rs` / `session_manager.rs` | 被 W2/W4 替代 |
| ironclaw `tools/builtin/sub_agent.rs` | W10 重写为 kernel adapter |
| 部分海外 channels/tools-src | 通过 Feature Flag 关闭,代码暂留 (W6 之后可按需真正下线) |

---

## 附录 A — 补盘 (v1.1 追加,解决 Q3 覆盖率缺口)

> **背景**: v1.0 覆盖率仅 ~75%,漏盘范围经用户审计指出。本附录**逐一表态**所有之前折叠/遗漏的 crate 和模块,覆盖率拉到 **~95%+**。

### A.1 codex-rs 完整 78 crate (v1.0 §1 漏的 18 个逐一表态)

#### 🔄 补 port (v1.0 漏掉的关键能力)

| crate | 状态 | Wave | 说明 |
|------|-----|-----|-----|
| `rollout` (独立, 非 rollout-trace) | 🔄 port | W9 | 会话"rollout"快照, 与 `dasclaw_rollout_trace` 合并 |
| `thread-store` | 🔄 port | W2 | 多线程对话持久化, 并入 `dasclaw_session` 存储层 |
| `state` | 🔄 port | W1 | Agent state 抽象, 并入 `dasclaw_agent_kernel::state` |
| `utils` | 🔄 随 W1 | W1 | 通用工具, 随 core 一起 port |
| `tools` (codex 独立 tools crate) | 🔄 port | W1 | 工具 trait 定义, 合入 `dasclaw_agent_kernel::tools` |
| `code-mode` | 🔄 port | W6 | 代码编辑模式状态机, 合入 `dasclaw_commands` |
| `shell-command` | 🔄 port | W0 | **安全关联**: shell 命令执行抽象, 必须与 `dasclaw_bash_guard` 对接 |
| `shell-escalation` | 🔄 port | W0 | **安全关联**: 提权命令识别, 合入 `dasclaw_bash_guard` |
| `secrets` (codex 独立 secrets crate) | 🔌 adapter | W0 | ⚠️ **与 ironclaw `src/secrets/` 重名**, codex 版并入 `dasclaw_secure_store` 或保留为 adapter 层 |
| `terminal-detection` | 🔌 随 TUI | — | TUI 辅助,仅 desktop-client 用时 port |
| `response-debug-context` | 🔄 port | W9 | **Q3 风险点**: LLM 出错诊断采样能力, 缺失将导致线上无诊断工具 — 必须 port |

#### ⚠️ 待决策 (补)

| crate | 决策点 |
|------|-------|
| `codex-api` / `codex-client` | desktop-client 用的是 claw-code API 还是 codex API? 二选一, MVP 建议选 claw-code (已熟悉) |
| `codex-experimental-api-macros` | 跟随 api 决策 |
| `tui` | 是否维护 CLI + TUI? 若 Y1 只做 desktop-client, 则 ❌ |
| `v8-poc` | V8 脚本引擎 POC, 产品 NorthStar 未提,Non-goal 但保留代码备用 |
| `uds` / `stdio-to-uds` | Unix domain socket 传输, 若 MCP 已覆盖则 ❌ |

#### 🔌 辅助 (补)

| crate | 状态 |
|------|-----|
| `test-binary-support` | 🔌 仅测试层保留 |
| `scripts` / `docs` / `vendor` | 🔌 非 Rust crate (辅助目录), 不计入盘点 |

### A.2 claw-code/rust/crates/runtime/src/ 42 个 .rs 文件逐一表态

v1.0 只说"runtime 作主 port 源", 实际内部文件应分开归属:

#### 🔄 W0 安全基线 (4 文件)

| 文件 | 目标 crate | 关键性 |
|-----|---------|-------|
| `bash.rs` + `bash_validation.rs` | `dasclaw_bash_guard` | 🔴 Bash 安全执行 baseline |
| `policy_engine.rs` | `dasclaw_policy` | 🔴 策略引擎核心 |
| `permission_enforcer.rs` + `permissions.rs` | `dasclaw_policy::permissions` | 🔴 权限执行 |
| `trust_resolver.rs` | `dasclaw_policy::trust` | 🟡 信任链解析 |

#### 🔄 W1-W4 内核 (10 文件)

| 文件 | 目标 crate |
|-----|---------|
| `prompt.rs` | `dasclaw_context_mgr::system_prompt` (PI baseline, [14 §4](14-claude-code-capability-parity.md)) |
| `session.rs` + `session_control.rs` | `dasclaw_session` |
| `conversation.rs` | `dasclaw_session::conversation` |
| `compact.rs` + `summary_compression.rs` | `dasclaw_context_mgr::compact` |
| `task_packet.rs` + `task_registry.rs` + `team_cron_registry.rs` | `dasclaw_tasks` |
| `config.rs` + `config_validate.rs` | `dasclaw_agent_kernel::config` |

#### 🔄 W5-W6 支撑 (14 文件)

| 文件 | 目标 crate |
|-----|---------|
| `file_ops.rs` | `dasclaw_apply_patch::ops` |
| `git_context.rs` + `branch_lock.rs` + `stale_base.rs` + `stale_branch.rs` | `dasclaw_git_utils` + `dasclaw_branch_guard` |
| `hooks.rs` + `plugin_lifecycle.rs` | `dasclaw_hooks_engine` + `dasclaw_plugins` |
| `mcp.rs` + `mcp_client.rs` + `mcp_lifecycle_hardened.rs` + `mcp_server.rs` + `mcp_stdio.rs` + `mcp_tool_bridge.rs` | `dasclaw_mcp` (6 种 transport 基线) |
| `lsp_client.rs` | `dasclaw_tools::lsp` |

#### 🔄 W7-W9 运维 (8 文件)

| 文件 | 目标 crate |
|-----|---------|
| `sandbox.rs` | `dasclaw_sandbox` (claw-code 路线作 fallback) |
| `bootstrap.rs` + `worker_boot.rs` | `dasclaw_agent_kernel::bootstrap` |
| `recovery_recipes.rs` | `dasclaw_rollout_trace::recovery` |
| `oauth.rs` | `dasclaw_secure_store::oauth` |
| `usage.rs` | `dasclaw_rollout_trace::usage` |
| `lane_events.rs` + `sse.rs` + `green_contract.rs` | `dasclaw_agent_kernel::streaming` |

#### 🔄 其他 (6 文件)

| 文件 | 目标 |
|-----|-----|
| `json.rs` | 工具, 随 core port |
| `remote.rs` | `dasclaw_agent_kernel::remote` |
| `lib.rs` | crate entry point |

### A.3 ironclaw src/ 23 个顶层模块逐一表态

v1.0 §3.2 只覆盖 13 模块, 以下是**之前漏的 10 个**:

| 模块 | 状态 | 说明 |
|-----|-----|-----|
| `agent/` | 🔌 adapter | ⚠️ 与新 `dasclaw_agent_kernel` 职责重叠! W1 必须确定:是直接替换还是留 adapter |
| `cli/` | ⚠️ 待决策 | 与 codex `cli` 命运绑定,若保留 CLI 则留 |
| `config/` | ✅ 保留 | 应用配置层 |
| `context/` | ❌ 下线 | 被 `dasclaw_context_mgr` 替代 |
| `db/` | ✅ 保留 | 数据库访问层 (libsql) |
| `document_extraction/` | ✅ 保留 | 文档提取能力 (产品差异化) |
| `estimation/` | ✅ 保留 | 成本估算 |
| `history/` | 🔌 adapter | 会话历史, W2 与 `dasclaw_session` 合并 |
| `hooks/` | 🔌 adapter | W6 合入 `dasclaw_hooks_engine` |
| `import/` | ✅ 保留 | 数据导入 |
| `llm/` | ✅ 保留 | LLM 提供商抽象层 (多家国产 LLM 适配) |
| `orchestrator/` | ✅ 保留 | 任务编排 (B 端差异化) |
| `pairing/` | ✅ 保留 | 设备配对 (desktop-client 特性) |
| `registry/` | ✅ 保留 | 工具/技能注册表 |
| `routines/` | ✅ 保留 | 定时任务 |
| `setup/` | ✅ 保留 | 初始化向导 |
| `testing/` | ✅ 保留 | 内建测试 helper |
| `tunnel/` | ✅ 保留 | 网络隧道 (可能与 `dasclaw_secure_store::network` 整合) |
| `webhooks/` | ✅ 保留 | Webhook 处理 |
| `workspace/` + `workspace_dir.rs` | ✅ 保留 | Workspace 管理 (与 `ironclaw_workspace_cap` 关联) |

### A.4 channels-src / tools-src 内部能力确认

| 模块 | ironclaw 专有逻辑 | 结论 |
|-----|----------------|-----|
| `channels-src/feishu` | ⚠️ 是否已集成 ironclaw DLP/审批链路? **未确认** | W0 启动前需抽查 |
| `channels-src/{discord,slack,telegram,whatsapp}` | ⚠️ 同上 | 同上 |
| `tools-src/*` | ⚠️ 是否用了 ironclaw `secrets/` 或 `redaction.rs`? **未确认** | W0 启动前需抽查 |

**行动项**: W0 启动前 1 天, 由 worker agent 跑 `rg "ironclaw_safety|secrets|redaction" channels-src/ tools-src/` 抽查并回写本节。

---

## 附录 B — 覆盖率重评

| 维度 | v1.0 | v1.1 (本版) |
|-----|------|-----------|
| codex-rs 78 crate 逐一表态 | 60/78 = 77% | **78/78 = 100%** (scripts/docs/vendor 3 个非 Rust 除外) |
| claw-code runtime 内部文件 | 0/42 = 0% (仅标 crate 级) | **42/42 = 100%** |
| ironclaw src/ 顶层模块 | 13/23 = 57% | **23/23 = 100%** |
| channels-src/tools-src 内部 | 0% | **登记未确认项, W0 前抽查** |
| **综合覆盖率** | ~75% | **~95%+** |

---

## 8. 变更历史

- **v1.1 (2026-04-24)**: 追加附录 A (补盘 18 codex crate + 42 runtime 文件 + 10 ironclaw 模块) + 附录 B (覆盖率重评)。响应用户 Q3 审计问题,覆盖率从 75% 拉到 95%+。**新发现 P0 关联 port**: `shell-command` + `shell-escalation` 必须与 W0 `dasclaw_bash_guard` 对接; `response-debug-context` 补 port 避免线上无诊断工具。
- **v1.0 (2026-04-24)**: 首版。三库 109 crate 完整盘点归属,与 [11 §8](11-target-architecture-route-b.md) Wave 路线图 100% 交叉验证无孤儿。新建 crate 数从原 14 修正为 **22** (沙箱细分 + W0 新增 + W6 细分)。
