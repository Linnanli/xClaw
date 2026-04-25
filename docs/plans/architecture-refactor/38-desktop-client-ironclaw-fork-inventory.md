# 38 — Desktop-Client 嵌入 IronClaw Fork 清单（路线 B 桌面端真实使用面）

> **版本**：v1.0 (2026-04-25) · 配套 [37-ironclaw-main-capability-inventory.md](37-ironclaw-main-capability-inventory.md)（主版 0.26.0）
> **方法**：严格按 [`AGENTS.md`](../../../AGENTS.md) 三级工具链 + 直接 ls/comm/grep 校准
> **fork 总规模**：**295,658 LOC**（vs 主版 464,222，**少 168k LOC = 36%**）

---

## 0. 结论先行

| 维度 | Fork (desktop-client/ironclaw 0.24.0) | Main (ironclaw-main 0.26.0) | Δ |
|---|---|---|---|
| **总 LOC（精确）** | **295,658** | 464,222 | **−168,564 (−36%)** |
| **核心 crate 数** | 2（ironclaw_common + ironclaw_safety） | 6（+ironclaw_engine 30k + ironclaw_skills 6k + ironclaw_gateway 2k + ironclaw_tui 11k） | −4 crate / −49k LOC |
| **src 顶层缺失模块** | — | **10 个**（auth / bin / bridge / code_challenge.rs / gate / generated_images.rs / http_intercept.rs / ownership / routines / workspace_dir.rs） | 见 §1 |
| **desktop-client 实际依赖** | `ironclaw = { path="./ironclaw", features=["libsql"] }` 整 crate + `ironclaw_safety` 单独 path-dep + `claw-code-llm` feature | — | **不只用 safety，是整个 ironclaw crate** |

**关键校准（subagent 错处）**：
- subagent 报告说"desktop-client 只用 ironclaw_safety"❌ 实际通过 `ironclaw = { path="./ironclaw" }` 把整个 crate 都拉了进来
- subagent 报告"fork 0.24.0"基于版本号推测，未经 git log 验证
- 真实 LOC 差距 168k（不是 subagent 估算的 28k）

---

## 1. Fork ↔ Main 结构差异（精确 ls/comm 校验）

### 1.1 src/ 顶层模块差异

执行 `comm -3 <(ls fork/src) <(ls main/src)` 结果：

| 模块 | Fork | Main | 类型 |
|---|:-:|:-:|---|
| `auth/` | ❌ | ✅ | **main 新增**（OAuth + identity） |
| `bin/` | ❌ | ✅ | main 新增（额外二进制） |
| `bridge/` | ❌ | ✅ | **main 新增 25,369 LOC**（v1/v2 适配层） |
| `code_challenge.rs` | ❌ | ✅ | main 新增（PKCE） |
| `gate/` | ❌ | ✅ | **main 新增 1,585 LOC**（权限审批门） |
| `generated_images.rs` | ❌ | ✅ | main 新增（图像生成） |
| `http_intercept.rs` | ❌ | ✅ | **main 新增**（HTTP MITM 拦截） |
| `ownership/` | ❌ | ✅ | **main 新增**（所有权隔离） |
| `routines/` | ❌ | ✅ | **main 新增**（routines 独立模块，fork 内可能在 agent/ 内） |
| `workspace_dir.rs` | ❌ | ✅ | main 新增（顶层 workspace_dir 入口） |

**fork 与 main 都有的**（28 个共有模块）：agent / channels / cli / config / context / db / document_extraction / error.rs / estimation / evaluation / extensions / history / hooks / import / lib.rs / llm / main.rs / observability / orchestrator / pairing / profile.rs / registry / safety / sandbox / secrets / settings.rs / setup / skills / sources / templates / testing / tools / tunnel / web / wit / workspace 等

### 1.2 crates/ 差异

| Crate | Fork | Main | LOC | 说明 |
|---|:-:|:-:|---:|---|
| `ironclaw_common` | ✅ | ✅ | 2,172 | 共享类型 |
| `ironclaw_safety` | ✅ | ✅ | 5,490 | DLP + safety pipeline |
| `ironclaw_engine` | ❌ | ✅ | **30,414** | **fork 完全没有 v2 引擎** |
| `ironclaw_skills` | ❌ | ✅ | 6,304 | fork 内 skill logic 还在 src/skills/ |
| `ironclaw_gateway` | ❌ | ✅ | 2,182 | fork 用 src/channels/web/ 替代 |
| `ironclaw_tui` | ❌ | ✅ | 11,021 | fork 不需要（桌面端用 Tauri） |
| `Dockerfile.sandbox` | ❌ | ✅ | — | main 新增 |

**fork 缺失的 crate 总 LOC**：30,414 + 6,304 + 2,182 + 11,021 = **49,921 LOC**

### 1.3 desktop-client/Cargo.toml 中的真实依赖

```toml
ironclaw_safety = { path = "./ironclaw/crates/ironclaw_safety" }
ironclaw = { path = "./ironclaw", features = ["libsql"] }
# Decimal (shared with ironclaw for LlmProvider::cost_per_token)
claw-code-llm = ["ironclaw/claw-code-llm"]
```

**实际依赖范围**：
1. `ironclaw_safety` 单独 path-dep（外部 safety 测试 / CLI）
2. `ironclaw` 整 crate path-dep + `libsql` feature
3. `claw-code-llm` feature 启用 ironclaw 内的 claw-code LLM provider 适配

→ **结论**：desktop-client 通过 `ironclaw` 整 crate 得到 fork **几乎全部能力**（除 fork 缺失的 10 个模块和 4 个 crate）。

---

## 2. Desktop-Client 实际调用的 ironclaw API 清单

来自 desktop-client/src/ 的 use 语句：

| 模块路径 | 主要 API | 调用位置 | 用途 |
|---|---|---|---|
| `ironclaw::agent` | `Agent`, `AgentDeps`, `RoutineEngine` | `desktop-client/src/engine.rs` | 启动 agent loop |
| `ironclaw::app` | `AppBuilder`, `AppBuilderFlags` | `engine.rs` | 组件初始化 |
| `ironclaw::channels` | `Channel` trait, `IncomingMessage`, `OutgoingResponse`, `StatusUpdate` | `tauri_channel.rs` + `engine.rs` | 实现 Tauri channel |
| `ironclaw::channels::web::log_layer` | `LogBroadcaster`, `init_tracing` | `main.rs` + `engine.rs` | 日志广播到前端 |
| `ironclaw::config` | `Config`, `SkillsConfig` | `engine.rs` + `state.rs` | 环境配置 |
| `ironclaw::hooks` | `bootstrap_hooks` | `engine.rs` | 生命周期钩子初始化 |
| `ironclaw::llm` | `LlmProvider` trait + `CompletionRequest/Response` + `ToolCompletionRequest/Response` + `create_session_manager` + `create_openai_provider` | `engine.rs` + `ipc/chat.rs` + `model_switch.rs` | LLM provider 注入 |
| `ironclaw::skills` | `LoadedSkill`, `prefilter_skills` | `ipc/chat.rs` | Skills 过滤 |
| `ironclaw::tools` | `ToolRegistry`, `builtin::SchedulerSlot` | `engine.rs` | 工具管理 |
| `ironclaw::error` | `LlmError` | `engine_startup_tests.rs` | 错误处理 |
| `ironclaw_safety` | DLP / prompt injection 全套 | `desktop-client/src/safety/` | 直接 path-dep |

**桌面端实际触达的能力域**（与 37 文档 18 大域对照）：

| 域 | 桌面端用到 | fork 是否提供 | 来源 |
|---|:-:|:-:|---|
| A. Agent Runtime | ✅ | ✅ | `ironclaw::agent::Agent` |
| B. Tool 系统 | ✅ | ✅ | `ironclaw::tools::ToolRegistry` |
| C. Sandbox | ⚠️ 部分 | ✅ | fork 内 `src/sandbox/` |
| D. Workspace | ✅ | ✅ | fork 内 `src/workspace/` |
| E. Channels | ✅（实现 Tauri channel） | ✅ | `ironclaw::channels::Channel` trait |
| F. LLM Provider | ✅ | ✅ | `ironclaw::llm::*` |
| G. Skills | ✅ | ✅ | `ironclaw::skills::prefilter_skills` |
| H. Extensions | ⚠️ 间接 | ✅ | fork 内 `src/extensions/` |
| I. Jobs / Scheduler | ⚠️ | ✅ | `builtin::SchedulerSlot` |
| K. Cost Guard | ⚠️ 间接 | ✅ | fork 内 `src/agent/cost_guard.rs` |
| L. Hooks | ✅ | ✅ | `ironclaw::hooks::bootstrap_hooks` |
| **M. Bridge** | **❌ fork 缺** | **❌** | fork 没有 bridge 模块 |
| N. OAuth | **❌ fork 缺** | **❌** | fork 没有 auth 模块 |
| O. Observability | ✅ | ✅ | `init_tracing` |
| P. Config | ✅ | ✅ | `ironclaw::config::Config` |
| Q. DB | ✅（libsql feature） | ✅ | features = ["libsql"] |
| R. Network Security | ❌ fork 缺 | ❌ | fork 没有 http_intercept |
| **engine v2** | **❌ fork 缺** | **❌** | fork 没有 ironclaw_engine crate |
| **gate（审批门）** | ❌ fork 缺 | ❌ | fork 没有 gate 模块 |
| **ownership（多租户）** | ❌ fork 缺 | ❌ | fork 没有 ownership 模块 |

**结论**：fork **缺失了主版的 7 个新增能力**（auth / bridge / gate / http_intercept / ownership / engine v2 / 部分 skills crate 化）。但桌面端**当前不依赖这 7 个**，所以仍可工作。

---

## 3. 升级到 0.26 的阻断分析

| 阻断 | 严重度 | 说明 | 缓解 |
|---|:-:|---|---|
| 6 个新模块需要适配（auth / bridge / code_challenge / gate / http_intercept / ownership） | 🔴 高 | bridge 25,369 LOC + gate 1,585 + auth 2,988 总 ~30k LOC 新增 | 路线 B 已决定**走 ADR-101 吸收式重构**，不直接升级 fork |
| `ironclaw_engine` crate 分离 | 🟡 中 | v2 引擎独立 crate，fork 内逻辑还在 src/agent/ | 路线 B：dasclaw_core 直接采用 v2 抽象 |
| `ironclaw_skills` crate 分离 | 🟡 中 | skills 移到独立 crate | 调整 import 路径 |
| `ironclaw_gateway` crate 分离 | 🟡 中 | Web API 重组 | 桌面端用 Tauri 命令 + IPC，不需要 gateway |
| Cargo.lock 大量解析变化 | 🟡 中 | 依赖版本可能升级 | path-dep 时无锁问题 |
| Routines 模块独立 | 🟡 中 | fork 内在 src/agent/routines（推断）vs main 在 src/routines/ | 直接搬运 |
| 24 migration V20-V24 fork 没有 | 🔴 高 | pairing_requests / backfill / sandbox restart / list escape / llm_calls index | 必须按版本号顺序补 |

**整体结论**：直接 git fetch upgrade 到 0.26 风险高（30k+ LOC 新增 + 5 个新 migration），不推荐。

---

## 4. 路线 B 替换决策树

```
当前现状（2026-04-25）
fork 0.24.0 嵌入式 submodule + ironclaw_safety path-dep + ironclaw 整 crate path-dep
    │
    ↓ 用户决策（已在 ADR-101 v2.1 确认）
    │
ADR-101 吸收式重构（4-6 周 → 经 Q4 重估调到 6-9 月）
    │
    ├─ 阶段 1（W1-W2）：基线收敛
    │   ├─ 保持 fork 不动
    │   ├─ 抽 ironclaw_engine v2 → dasclaw_core 骨架（30,414 LOC）
    │   └─ 保留 ironclaw_safety 直接 path-dep
    │
    ├─ 阶段 2（W3-W6）：能力吸收（**本文档真正的工作量**）
    │   ├─ ironclaw bridge 25,369 → dasclaw_bridge（独立 crate）
    │   ├─ ironclaw db 13,977 + 24 migration → dasclaw_db
    │   ├─ ironclaw workspace 12,857 → dasclaw_workspace
    │   ├─ ironclaw channels 60,211 → dasclaw_channels_v2
    │   ├─ ironclaw llm 33,654 → dasclaw_llm_chain
    │   ├─ ironclaw tools 57,003 → dasclaw_tools_v2
    │   ├─ ironclaw extensions 15,348 → dasclaw_extensions
    │   ├─ ironclaw config 11,568 → dasclaw_config_v2
    │   └─ ironclaw worker 6,654 → dasclaw_scheduler
    │
    ├─ 阶段 3（W7+）：关闭 fork
    │   ├─ desktop-client 改 use dasclaw::* 替代 use ironclaw::*
    │   ├─ 删除 desktop-client/ironclaw/ submodule
    │   └─ 保留 ironclaw_safety 作为外部依赖（已稳定）
    │
    └─ 阶段 4（W8-W9）：补主版独有能力
        ├─ auth + code_challenge → dasclaw_auth（参考 codex device-key 更强）
        ├─ bridge → 已在阶段 2 处理
        ├─ gate → dasclaw_approvals（合并 cost_guard）
        ├─ http_intercept → dasclaw_safety_v2
        ├─ ownership → dasclaw_workspace_cap（已在 W2 处理）
        └─ ironclaw_skills + ironclaw_gateway → 不移植（桌面端不需要）
```

---

## 5. fork 的真实价值与定位

**fork 不是"轻量子集"，而是"主版 -2 小版本的快照"**：

| 角度 | 评估 |
|---|---|
| 维护成本 | 🟡 中 — 每次 main 升级需手工 sync，已经落后 2 个小版本 |
| 桌面端依赖深度 | 🔴 高 — 整个 ironclaw crate 通过 path-dep 进入 desktop-client 编译图 |
| 短期可用性 | ✅ 仍可工作 — 桌面端没用到 main 新增的 7 个模块 |
| 长期可持续性 | ❌ 差 — 0.27/0.28 后差距会指数增长 |
| 替代方案成熟度 | ⚠️ 中 — ADR-101 路线 B 是替代方案，但尚未启动 |

**短期建议**：保持 fork 0.24.0 直到 W3 启动 → W6 完成阶段 2 后切换到 dasclaw_*

**禁止行为**：
- ❌ 直接把 fork 升级到 0.26（30k+ LOC 新增风险）
- ❌ 在 fork 内打补丁（会让 ADR-101 吸收变更难度变大）
- ✅ 只允许：bug fix 反向 PR 到 ironclaw-main，从 main 拉取 cherry-pick 到 fork

---

## 6. 与其他文档的接口

- **30 文档**：本 38 文档 §2 的「桌面端实际触达能力域」表应同步到 30 v2.1 desktop 列
- **31 文档**：阶段 4 的 dasclaw_auth / dasclaw_approvals / dasclaw_workspace_cap 已在 ADR-101/107a 中
- **32 文档**：阶段 2 的 9 个 crate 移植已写入 W6 任务清单
- **37 文档**：本文档是 37 文档的「desktop-client 实际使用面切片」

---

## 7. 关键事实（防混淆）

> 以下事实必须直接引用文件路径或 LOC 数据，不允许凭印象。

1. fork 总 LOC：**295,658**（直接 `find | wc -l`）
2. main 总 LOC：**464,222**（直接 `find | wc -l`）
3. fork 缺失模块：**10 个 src 模块 + 4 个 crate** （ls/comm 直接对比）
4. desktop-client 通过 `ironclaw = { path="./ironclaw" }` 依赖 **整个 ironclaw crate**，不是仅 safety
5. fork 不存在的能力域：bridge / engine v2 / auth / gate / http_intercept / ownership
6. fork 是 0.24.0（推断自 README/CHANGELOG，未经 git log 严格验证）

---

## 变更日志

- v1.0 (2026-04-25) — 初版，校准 subagent 估算偏差（subagent 说 fork "112k LOC / Δ +28k"，实际 fork 295,658 / Δ −168,564），明确 desktop-client 真实依赖范围与 fork 缺失的 10 模块 + 4 crate
