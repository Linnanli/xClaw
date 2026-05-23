# ADR-155：F4.4 orchestrator 落点决策

- 状态：Proposed
- 提案人：Coding Agent（基于 ADR-152 §3 F4.4 留空条款 + ADR-153 无头 agent 框架定位）
- 关联：ADR-152（agent and capability fusion）§3 阶段 F4 / §11.9 / ADR-153（headless agent framework）§4.2 / ADR-129（verbatim port mandate）§1.3
- 时间：2026-05-21

---

## 1. 背景

ADR-152 §3 阶段 F4 列出五项 desktop 模块下沉清单，其中 F4.4 写的是：

> **F4.4**：`orchestrator` 落点决策 + 搬迁（3.3K LoC）。需补 ADR 决定落点（候选：`dasclaw_runtime::orchestrator` 或新 crate）。

也就是说 F4.4 在 ADR-152 落笔时就是一个**显式留空的占位**，明确要求后续单独写 ADR 决定 orchestrator 该落在哪里、要不要拆。本 ADR 就是来填这个坑的。

同时 ADR-153 已经把 x-claw 的演化目标定成「无头 agent 框架」，§4.2 步骤 2 写「从 ironclaw 的 agent/ + orchestrator/ 抽出最小 agent loop」，但同时声明无头 agent **不带 HTTP、不带数据库、不带租户**。这两段加起来意味着：F4.4 不能只是简单地把整个 orchestrator 模块原样搬到 `dasclaw_runtime::orchestrator`，必须先回答一个更根本的问题——orchestrator 现在做的事，**对无头 agent 是不是必需**。

## 2. orchestrator 现在到底是什么

读完 `desktop-client/ironclaw/src/orchestrator/` 5 个文件 3330 行后的真实职责盘点：

| 文件 | 行数 | 职责 |
|------|------|------|
| `mod.rs` | 332 | `setup_orchestrator()` 启动入口，`OrchestratorSetupError`（Docker 缺失检测），端口解析 |
| `job_manager.rs` | 745 | `ContainerJobManager`：建/停/列容器，`JobMode`（Worker / SubAgent / 已 deprecated 的 ClaudeCode），`ContainerJobConfig`，绑定 `bollard` 操作 Docker |
| `api.rs` | 991 | `OrchestratorApi`：axum router，跑在 :50051 端口，给容器内的 worker 提供 LLM proxy / job 描述 / 状态回传 / 凭据下发 / 事件上报 |
| `auth.rs` | 293 | `TokenStore` + `CredentialGrant`：per-job bearer token + per-job 凭据授权清单，constant-time 校验 |
| `reaper.rs` | 969 | `SandboxReaper`：定时扫描带 `ironclaw.job_id` label 的 Docker 容器，孤儿清理 |

**一句话概括**：orchestrator 是 ironclaw 桌面后端的「容器化沙箱执行后端」——主进程把"跑一个 worker"这件事委托给 Docker 容器，自己开一个 HTTP API 当 worker 的「外部世界代理」（LLM、凭据、事件）。

## 3. 三方对账：codex / claw-code 有没有同类能力

按 AGENTS.md 任务启动 4 问（涉及跨项目对账 + 否定性结论），必须三层验证。

**Level 1 + 2（code-review-graph cross repo）**：

- `cross_repo_search("ContainerJobManager", limit=15)` → 命中 30 处，**全部在 ironclaw + ironclaw-main**（后者是 fork 同源副本），codex / claw-code 0 命中
- `cross_repo_search("SandboxReaper", limit=15)` → 命中 12 处，全部在 ironclaw 系，codex / claw-code 0 命中
- `cross_repo_search("TokenStore", kind=Class)` → 仅 ironclaw 系命中

**Level 3（字面量 grep）**：

- `grep -rln 'orchestrator' codex-cli-main/codex-rs/` → 命中文件全部在 `rmcp-client/` / `core/src/tools/runtimes/` 里，是 MCP 客户端注释和工具运行时，与"容器编排"无关；codex 整个 codebase 没有"orchestrator/container job manager"概念
- codex 的 sandbox 是 `core/src/sandboxing/`（Landlock/Seatbelt）+ `execpolicy/` + `sandboxing/`——**进程级 syscall 沙箱**，单进程内开子进程，不开容器、不起 HTTP、不发 token
- claw-code 的 `rust/crates/runtime/src/worker_boot.rs` 是**进程级 worker 状态机**（Spawning / TrustRequired / ReadyForPrompt / Running / Finished / Failed），监控 CLI 子进程的握手，**不涉及容器、不发 HTTP**

**结论**：

- ironclaw 的 orchestrator（container + HTTP + bearer token + reaper）在 codex / claw-code 中**没有等价实现**
- 这不是"换了名字"的同源能力，而是 ironclaw 独有的架构选择
- 因此搬迁路径上**不能复用 codex / claw-code 的代码**，只能在 ironclaw 内部决定如何重新摆放

## 4. 关键判断：orchestrator 对无头 agent 是不是必需

把 ADR-153 §4.2 的最小可用形态拿出来对照：

```rust
let agent = Agent::builder()
    .llm(llm)
    .workspace(std::env::current_dir()?)
    .tools_default()
    .hooks_default()
    .build()?;
let result = agent.run("帮我把 README 里所有 TODO 列出来").await?;
```

这个 Agent **不需要**：

- Docker（用户机器上可能没装）
- :50051 HTTP API（自己进程内函数调用就够）
- per-job bearer token（没有跨进程边界，无需鉴权）
- SandboxReaper（没有容器要 reap）
- `ContainerJobConfig`（用户传 `workspace + tools` 已经够）

orchestrator 解决的问题是「**主进程要把 agent 工作丢给一个隔离环境跑**」。无头 agent 框架是「**调用方自己就是 agent，在自己的进程里跑**」——这是两种部署形态，不是同一个能力的两种实现。

**直接结论**：orchestrator 整套（container + HTTP + token + reaper）**不属于** `dasclaw_runtime` 应该承载的内容。把它塞到 `dasclaw_runtime::orchestrator` 会让"一个库 + 几行代码就能跑 agent"的承诺破产——任何引入 `dasclaw_runtime` 的下游 crate 都会被迫拉上 `bollard` / `axum` / Docker 运行时依赖。

## 5. 候选方案与权衡

### 候选 A：搬到 `dasclaw_runtime::orchestrator`（ADR-152 §3 原候选之一）

- ✓ 符合 ADR-152 §3 F4.4 原文给的两个候选之一
- ✗ 违反 ADR-153 §4.2「dasclaw_runtime 不带 HTTP、不带数据库、不带租户」
- ✗ 强制下游 crate 拉 bollard + axum 重依赖
- ✗ 在无头 agent 用例中 100% 死代码

**否决**。

### 候选 B：新开 `crates/dasclaw_orchestrator` crate（ADR-152 §3 原候选之二）

- ✓ 隔离 bollard / axum 依赖，调用方按需引入
- ✓ verbatim 搬迁，最贴近 ADR-129 §1.3
- ✗ 但 orchestrator 的依赖图里还有 `crate::channels::web::types::ToolDecisionDto` / `crate::db::Database` / `crate::worker::api::*` / `crate::sandbox::connect_docker` / `crate::bootstrap::dasclaw_base_dir` / `crate::error::OrchestratorError` / `ironclaw_common::AppEvent` 共 7 处 desktop 反向依赖
- ✗ 同样触发 ADR-152 §11.9 的「禁止反向 path 依赖」红线——和 F4.2 routines 评估文档（`f42-preflight-dependency-assessment.md`）里指出的症结一模一样
- ✗ 强行 verbatim 搬迁就是把 desktop 的 7 个未下沉模块全部拖到新 crate 里，违反原则的方向，但绕不开

**短期不可行**。要落地必须先下沉 `channels::web::types::ToolDecisionDto` / `worker::api::*` / `sandbox::connect_docker` / `db::Database` / `bootstrap::dasclaw_base_dir` / `ironclaw_common::AppEvent` 这 6 个模块，每个都需要单独 ADR + PR。

### 候选 C：orchestrator 留在 desktop-client，作为**桌面后端独占模块**

- ✓ 符合 ADR-153 §2 现状盘点表里"桌面服务皮"和"运行时编排"的分类逻辑：orchestrator 既然依赖 db / channels / worker / sandbox / ironclaw_common，它**本质上就是桌面服务皮的一部分**
- ✓ 不破坏 verbatim 红线（不搬不改）
- ✓ 不强迫 dasclaw_runtime 拉 bollard / axum
- ✓ 不需要前置下沉 6 个 desktop 模块
- ✗ 但要正式撤销 ADR-152 §3 F4.4 的"必须搬"承诺，把它显式标为"留在桌面"
- ✗ 三方对账已经证明 codex / claw-code 没用这套架构，留在桌面也意味着这部分代码后续没有「上游融合」红利

**推荐**。

### 候选 D：orchestrator 拆成两半——把跨语言无依赖的部分（`auth.rs::TokenStore`、`auth.rs::CredentialGrant`、auth middleware）下沉到 `dasclaw_runtime::auth_token`，其余留在 desktop

- ✓ 部分释放 ADR-152 §3 F4.4 的搬迁义务
- ✓ `auth.rs` 293 行**实际仅依赖 axum + tokio + uuid + subtle + serde**，没有 crate-内反向依赖（已 grep 验证）
- ✓ 拆分后剩余 3037 行仍按候选 C 留在桌面
- ✗ 但 `TokenStore` 只为 orchestrator 服务，单独抽到 dasclaw_runtime 后没有第二个使用方，违反「不为单一使用方建模」的设计准则
- ⚠ 短期无收益、长期也无明确使用场景

**不推荐**——除非 `TokenStore` 出现第二个使用方（例如未来 ADR-153 步骤 3 的 `dasclaw_session` 要做跨进程会话鉴权）。在出现第二个使用方之前，强行下沉就是凭空多养一个 crate。

## 6. 决策

**采用候选 C**：orchestrator 留在 `desktop-client/ironclaw/src/orchestrator/`，作为 ironclaw 桌面后端的独占模块，正式撤销 ADR-152 §3 F4.4 的"必须搬到 crates/"承诺。

理由按重要度排序：

1. **架构匹配**：orchestrator 解决"主进程把工作丢给容器跑"的部署形态，与 ADR-153 §4.2 定义的「调用方自己就是 agent」无头部署形态正交，搬到 dasclaw_runtime 会污染框架定位
2. **三方对账**：codex / claw-code 都没这套，搬到 crates/ 不能换来跨上游融合红利
3. **依赖现实**：orchestrator 反向依赖 desktop 6 个未下沉模块（db / channels::web / worker / sandbox / bootstrap / ironclaw_common），verbatim 搬迁不可行，违反 ADR-152 §11.9
4. **代价对称**：F4.2 routines 已经在 `f42-preflight-dependency-assessment.md` 走过同一条结论路径（候选 C「暂缓 / 不搬」），保持一致

## 7. 配套动作

1. **修订 ADR-152 §3 F4.4 条款**：把"`orchestrator` 落点决策 + 搬迁（3.3K LoC）"改为"`orchestrator` 留在 desktop-client，理由见 ADR-155"。该修订**不在本 ADR PR 范围**，留待单独 PR
2. **不改任何代码**：orchestrator 5 个文件、3330 行保持原样
3. **下游契约**：orchestrator 公共 API（`ContainerJobManager` / `SandboxReaper` / `TokenStore` / `OrchestratorApi`）的调用方仍是 desktop-client 内部（`main.rs` / `app.rs` / `tenant.rs` / `channels/web/`），无需改写
4. **ADR-153 §2 现状盘点表更新**：在「运行时编排」类别下给 `orchestrator/` 加一行说明「按 ADR-155 留桌面」。同样**不在本 ADR PR 范围**

## 8. 后续路径

如果未来真有第二个使用场景（例如 dasclaw_cli 想用容器化 sandbox），可以：

- 重新走候选 D：把 `auth.rs` 抽到 `dasclaw_runtime::auth_token`
- 或者走候选 B：先批量下沉 6 个前置 desktop 模块，再 verbatim 搬整个 orchestrator

但这两条路径都**不在本 ADR 范围**，需要新 ADR 触发。

## 9. 不在本 ADR 范围

- ADR-152 §3 F4.4 条款文本如何修订（留待 ADR-152 自身修订 PR）
- ADR-153 §2 现状盘点表如何更新（同上）
- desktop-client/ironclaw 的 orchestrator 自身的代码质量问题（is-`#[deprecated]` 的 `JobMode::ClaudeCode` 何时彻底删除等），这是 desktop 内部演化，与下沉决策无关

## 10. Sources read

- [`adr-152-agent-and-capability-fusion.md`](adr-152-agent-and-capability-fusion.md) §3 阶段 F4 / §11.9 路径倒置
- [`adr-153-headless-agent-framework-draft.md`](adr-153-headless-agent-framework-draft.md) §2 现状盘点 / §4.2 步骤 2
- ADR-129 §1.3 verbatim port mandate（按 AGENTS.md 与 ADR-153 / ADR-154 惯例引用，不附物理链接）
- [`f42-preflight-dependency-assessment.md`](f42-preflight-dependency-assessment.md) 候选 C 路径模式参考
- `desktop-client/ironclaw/src/orchestrator/mod.rs`（332 行）
- `desktop-client/ironclaw/src/orchestrator/job_manager.rs`（745 行，头 100 行）
- `desktop-client/ironclaw/src/orchestrator/api.rs`（991 行，头 80 行）
- `desktop-client/ironclaw/src/orchestrator/auth.rs`（293 行，头 60 行）
- `desktop-client/ironclaw/src/orchestrator/reaper.rs`（969 行，头 60 行）
- code-review-graph `cross_repo_search` for ContainerJobManager / SandboxReaper / TokenStore（确认 codex / claw-code 0 命中）
- 字面量 grep `orchestrator|ContainerJobManager|SandboxReaper|bollard` 跨 codex-cli-main / claw-code（确认 codex 命中为 rmcp/tools/runtimes 注释，与容器编排无关；claw-code worker_boot 是进程级 worker 状态机）
- crates/ 沙箱栈交叉验证（详见 §11 附录）

---

## 11. 附录：crates/ 沙箱栈现状交叉验证

为了进一步确认 ADR-155 决策的合理性，对 `crates/` 工作区现有的沙箱实现做一轮独立核验，回答两个问题：

> **问题 1**：`crates/` 里现有的"建沙箱"代码路径，有没有 Docker 选项？
>
> **问题 2**：如果没有，那 `crates/` 现在用的是什么沙箱机制？

### 11.1 验证方法

- 字面量 grep `bollard` 跨 `crates/**/Cargo.toml`
- 字面量 grep `docker|Docker|container|Container` 跨 `crates/**/*.rs`
- 函数签名 grep `pub fn.*sandbox|landlock|seatbelt|appcontainer|spawn_sandboxed` 跨 `crates/dasclaw_sandbox*/src/**/*.rs`

### 11.2 验证结果

**结论 A：`bollard`（Docker Rust SDK）在 `crates/` 零依赖**。所有 `crates/*/Cargo.toml` 中 `bollard` → 0 命中。

**结论 B：`crates/` 现成沙箱实现是三套进程级 OS 沙箱**：

| crate | 沙箱机制 | 关键文件 |
|-------|---------|---------|
| `dasclaw_sandbox_linux` | bwrap (bubblewrap) + Landlock | `src/bwrap.rs`（注释明确写"mirrors the semantics used by the macOS Seatbelt sandbox"）|
| `dasclaw_sandbox_windows` | Win32 AppContainer + JobObject + 受限令牌 + ACL | `src/lib.rs` §51"Restricted-token builders for AppContainer / capability sandboxing" / `setup_main_win.rs` |
| `dasclaw_sandboxing` | 跨平台统一抽象层 | （trait 层）|

**结论 C：`crates/` 里看似跟 "container" / "docker" 沾边的命中全是误报或语义不同**：

| 命中位置 | 真实含义 | 与 Docker 关系 |
|---------|---------|---------------|
| `dasclaw_sandbox_windows::acl::CONTAINER_INHERIT_ACE` | Win32 ACL 继承标志位（常量 `0x2`） | 无关 |
| `dasclaw_sandbox_windows::lib.rs` §51 "AppContainer" | Windows 原生 AppContainer 沙箱机制 | 无关（同名不同物）|
| `dasclaw_governance::tool_visibility::ToolVisibility::Container` | 工具可见性枚举值，标注"Sandboxed container worker"作为审批级别 | 仅是标签，不建容器 |
| `dasclaw_core::agentic_loop` / `hooks.rs` 注释 "container context" | 泛指 LoopDelegate 抽象下游可能实现的执行上下文 | 无关（泛指语义）|
| `dasclaw_bash_validation` / `dasclaw_bash_permissions::"docker"` | 把字符串 `docker` 当作命令名做权限/路径校验 | 当外部命令对待 |
| `dasclaw_sandbox_windows::setup_orchestrator.rs::".docker"` | 用户家目录下 `.docker/` 路径，作为路径白名单 | 当文件系统路径对待 |
| `dasclaw_protocol::models.rs` "container.exec" | 协议字段名（OpenAI Codex container 工具名），是上游协议词 | 无关 |

**结论 D：真正的 Docker 容器编排只存在于 `desktop-client/ironclaw/src/orchestrator/job_manager.rs`**，整个 `crates/` 工作区**没有一行 bollard 调用**。

### 11.3 对决策的影响

本验证进一步加固第 6 节决策：

- `crates/` 沙箱栈的**技术路线是进程级 OS 沙箱**（Landlock / Seatbelt / AppContainer），不是容器化路线
- ironclaw 的 orchestrator 是**桌面后端历史路线**（Docker 容器化 worker），与 `crates/` 现有沉淀的路线**异构**
- 把 orchestrator 强行下沉到 `crates/` 会同时引入**两套异构沙箱栈**（进程级 + 容器级），架构上更难自洽，与"统一抽象"目标相悖
- 留 orchestrator 在 desktop-client 同时给"两条沙箱路线分层"留出空间——`crates/` 走进程级路线服务无头 framework 与 codex 上游融合，desktop-client 走 Docker 容器路线服务自己的桌面后端

### 11.4 未来路径

如果未来出现"服务端部署需要 Docker 沙箱"的新场景（详见 issue #752），应当作为一个**新的部署形态**来设计，而不是把现有 desktop orchestrator 直接下沉。具体路径留待届时新 ADR 触发。
