# ADR-134: codex `shell-escalation` adoption evaluation (research-only)

- **Status**: 🔴 **Decision: NON-GOAL — do not port** (research-only ADR per [#326](https://github.com/Linnanli/xClaw/issues/326) Part 3b)
- **Date**: 2026-05-08
- **Approver**: pending nally sign-off
- **Authors**: GitHub Copilot agent
- **Tracker**: [#326](https://github.com/Linnanli/xClaw/issues/326) Part 3b — `shell-escalation` 评估研究 (B-6 + C-6)
- **Related**:
  - [ADR-129](adr-129-sandbox-windows-windows-crate-adoption.md) §1.3 — verbatim red line
  - [ADR-130](adr-130-sandbox-windows-lib-bin-split.md) — Windows sandbox lib/bin split
  - [ADR-131](adr-131-windows-job-object-resource-limits-wrapper.md) — Windows process containment 路径
  - [ADR-43](43-net-proxy-port-adr.md) / [ADR-44](44-net-proxy-port-completion.md) — 现有 proxy 模型（已落地，覆盖网络逃逸需求）
  - [ADR-118](adr-118-claw-code-readonly-and-self-impl.md) — claw-code readonly + self-impl 边界
  - [doc 32 §W4-§W6](32-execution-plan.md) — 落地 wave 决策点
  - codex 上游：`codex-cli-main/codex-rs/shell-escalation/`（Unix-only，2,217 LOC + bin）

---

## 1. Context

### 1.1 上游事实（已三层验证）

`codex-cli-main/codex-rs/shell-escalation/` 是一个 **Unix-only**（`#[cfg(unix)]` 全模块门控）的 setuid 风格命令拦截器。验证：`semantic_search "EscalateServer"` → `vscode_listCodeUsages` on `EscalateServer` → `rg "use codex_shell_escalation"`。

| 文件 | LOC | 角色 |
|---|---|---|
| `lib.rs` | 35 | 全部 `#[cfg(unix)]` 出口 |
| `unix/mod.rs` | 79 | 模块根 + 文档（"patched shell invokes an exec wrapper on every exec() attempt"）|
| `unix/escalate_server.rs` | **1,064** | `EscalateServer` 主体 — Unix domain socket 服务端，处理 `Run` / `Escalate` 决策 |
| `unix/socket.rs` | 523 | UDS / SCM_RIGHTS 文件描述符传递 |
| `unix/escalate_client.rs` | 144 | 客户端（exec wrapper 调用方） |
| `unix/escalate_protocol.rs` | 88 | wire format（`EscalateRequest`, `EscalateResponse`） |
| `unix/stopwatch.rs` | 237 | 计时器 |
| `unix/escalation_policy.rs` | 14 | policy enum |
| `unix/execve_wrapper.rs` | 25 | execve 包装入口（被 patched shell 调用） |
| `bin/` | — | 独立二进制 `codex-shell-escalation` |

### 1.2 工作模型（来自 `unix/mod.rs:1-50` 上游文档）

> A patched shell invokes an exec wrapper on every `exec()` attempt. The wrapper sends an `EscalateRequest` over the inherited `CODEX_ESCALATE_SOCKET`, and the server decides whether to run the command directly (`Run`) or execute it on the server side (`Escalate`).

要求三件事同时存在：

1. **patched shell** — 每次 `exec()` 都被劫持。codex 实现是用 `LD_PRELOAD` 注 + 自定义 shell 二进制（依赖发行版 toolchain）。
2. **Unix domain socket + SCM_RIGHTS** — 跨进程传 fd（macOS / Linux 才有；Windows 没有原生等价）。
3. **escalation server 进程**作为 sandbox 出口的"决策者"，需要常驻，且要管理多并发请求。

整体形态接近 macOS sudo 的 ASKPASS 模型，但更复杂 — 是一个"exec 时通过 socket 决策能否真的 exec"的 fail-closed 拦截。

### 1.3 dasclaw 现状

dasclaw 全 workspace `rg "EscalateServer\|shell_escalation\|CODEX_ESCALATE_SOCKET"` 命中数：**0**（仅 codex 上游 vendored 副本）。无任何 dasclaw crate 试图调用、复用、stub 此模型。

桌面客户端的安全模型走的是：

| 层 | 实现 | ADR |
|---|---|---|
| 命令执行边界 | `dasclaw_governance` 路由 + `dasclaw_bash_validation` 门控 | claw-code-derived |
| 网络出口控制 | `dasclaw_net_proxy`（HTTP forward proxy + MITM CA）| [ADR-43](43-net-proxy-port-adr.md) / [ADR-44](44-net-proxy-port-completion.md) |
| 进程隔离 | Linux: cgroup v2 + bubblewrap；macOS: seatbelt; Windows: AppContainer + Job Object | [ADR-45](45-resource-limits-adr.md) / [ADR-129](adr-129-sandbox-windows-windows-crate-adoption.md) / [ADR-131](adr-131-windows-job-object-resource-limits-wrapper.md) |
| 策略层 | `dasclaw_execpolicy`（Starlark，[ADR-132](adr-132-execpolicy-starlark-port-plan.md)）|
| 命令解析（UI）| `dasclaw_shell_command`（[ADR-133](adr-133-shell-command-adoption-eval.md)）|

**已经覆盖 codex shell-escalation 想解决的问题**（"只让 sandbox 里的命令以受控方式 exec"），通过 OS 原生隔离 + 命令边界门控 + 网络代理三层组合。

---

## 2. Decision

### 2.1 决策：**NON-GOAL — 不 port，不 stub，不 vendor 调用**

`shell-escalation` **不**进入 dasclaw 范围。理由：

#### 2.1.1 平台不对称（致命）

- shell-escalation 全模块 `#[cfg(unix)]`，**Windows 完全无落地路径**。dasclaw 桌面客户端是跨平台一等公民（含 Windows，参 ADR-129/130/131）。
- 仅 Linux/macOS 启用此机制 = 三个 OS 三套安全语义，违反 [doc 51 cross-platform support matrix](51-sandbox-cross-platform-support-matrix.md)。

#### 2.1.2 实施成本与收益严重不对称

- 实施成本：
  - 2,217 LOC verbatim port
  - 还要 vendor codex 的 patched shell 二进制（不在 `shell-escalation` crate 内，是配套基建）
  - LD_PRELOAD / SCM_RIGHTS 调试在 Linux 不同发行版（musl vs glibc）+ macOS 三个 toolchain 都要分别验证
  - `dasclaw_sandbox` Linux/macOS 后端必须重写以接入 escalate socket
- 收益：
  - 当前 `dasclaw_sandbox` Linux 走 cgroup v2 + bubblewrap 已经达到 codex 同等隔离强度（[doc 41](41-docker-vs-os-sandbox-capability-comparison.md)）；
  - codex 用 escalation 主要是为了在不依赖 root namespace 的环境（GitHub Codespaces / Replit）做隔离 — 这不是 dasclaw 桌面客户端的目标场景。

#### 2.1.3 与现有架构正交但语义重叠

- `dasclaw_governance` 已经在 process boundary 决策；再叠一层 escalation 等于双门控，会出现"governance 放行但 escalation 拒绝"或反向的语义冲突。
- `dasclaw_execpolicy`（[ADR-132](adr-132-execpolicy-starlark-port-plan.md)）是 dasclaw 钦定的策略中心，不应被 escalation 旁路。

### 2.2 不做的事

- ❌ 不 port `crates/dasclaw_shell_escalation`（不创建 crate）
- ❌ 不在 `dasclaw_sandbox` 留 stub trait 等待未来对接
- ❌ 不 vendor `EscalateServer` 模式作"参考实现"
- ❌ 不在 `Cargo.toml` 加 path-dep 到 codex 上游 vendored 副本

### 2.3 替代路径（如未来出现需求）

如果将来桌面客户端需要"exec-time 拦截"（例如运行不可信 binary，且 cgroup/AppContainer/seatbelt 不足）：

| 替代 | 优势 | 缺点 |
|---|---|---|
| **Linux**: bubblewrap + seccomp filter | 已是 dasclaw_sandbox Linux 后端 | seccomp 表达力有限 |
| **macOS**: 升级 seatbelt profile + Endpoint Security framework | 苹果原生，Apple Silicon 友好 | 需要内核扩展授权（开发者账户） |
| **Windows**: 已用 AppContainer + Job Object（ADR-129/131），扩展 token restriction | 已落地 | — |

任一替代都比 verbatim port codex shell-escalation 工作量小、跨平台一致性好。出现需求时单独提 ADR-1XX，**不**复用 codex 此模块。

### 2.4 mechanical guard

落地 PR（即本 PR）顺便把 `crates/dasclaw_shell_command/`（ADR-133 待落地）和 `dasclaw_execpolicy`（ADR-132 待落地）一起加入 ADR-129 风格的 drift guard，但 **不为 shell-escalation 单独添加 guard** — 因为 dasclaw 不依赖它，无飘移可言。

---

## 3. Consequences

### 3.1 Positive

- **明确的 non-goal 信号** — 后续 contributor / agent 看到 `codex-cli-main/codex-rs/shell-escalation/` 时立即知道"研究过，不进 dasclaw"，不会重复评估。
- **跨平台一致性保留** — 桌面客户端在三 OS 上走同一套 sandbox + governance + proxy + execpolicy 模型，无 Linux/macOS-only 例外。
- **架构面更小** — 少 2,200 LOC + 2 个 systemd-style 长驻服务（escalate server + patched shell）。

### 3.2 Negative

- **如果未来 dasclaw 改方向**（容器化 / 远端运行），exec-time 拦截需求出现时无法直接复用 codex 现成模块；不过 §2.3 已列出三 OS 替代路径。
- 与 codex 主线渐行渐远（codex 仍把 escalation 作为核心安全机制）— 这是显式取舍，不是 bug。

---

## 4. Implementation sequence

**无**。本 ADR 决策为 NON-GOAL，无落地代码。仅做以下文档同步（已包含在本 PR 中）：

1. doc 32 §W4 / §W6 添加"shell-escalation = non-goal"明文条款，链接本 ADR
2. doc 35（codex capability inventory）shell-escalation 相关条目（如有）标注 `不 port`

无 CI 改动，无 Cargo.toml 改动。

---

## 5. Rejected alternatives

### 5.1 verbatim port（套用 ADR-129 §1.3 模式）

- **拒绝理由**：见 §2.1，平台不对称 + 收益不对称。

### 5.2 stub-only port（仅出 trait + `unimplemented!`）

- **拒绝理由**：违反 `scripts/check_no_panics.py` 红线（生产代码不允许 `panic!`/`unimplemented!`）；且 stub 会诱导 contributor 误以为 dasclaw 计划支持。

### 5.3 仅 port `escalate_protocol.rs`（88 LOC wire format）作为协议储备

- **拒绝理由**：协议 byte 层与 patched shell + execve_wrapper 强耦合，单独 port 协议没有任何使用方。

---

## 6. References

- codex-rs/shell-escalation/src/unix/mod.rs:1-50 — 上游模型描述
- [ADR-43](43-net-proxy-port-adr.md) / [ADR-44](44-net-proxy-port-completion.md) — 网络出口控制（替代 escalation 的网络维度）
- [ADR-129](adr-129-sandbox-windows-windows-crate-adoption.md) — Windows 进程隔离（替代 escalation 的进程维度）
- [ADR-131](adr-131-windows-job-object-resource-limits-wrapper.md) — Job Object containment
- [ADR-132](adr-132-execpolicy-starlark-port-plan.md) — Policy 中心（替代 escalation 的策略维度）
- [ADR-133](adr-133-shell-command-adoption-eval.md) — 命令解析维度
- [doc 51](51-sandbox-cross-platform-support-matrix.md) — 跨平台一致性原则
- [#326](https://github.com/Linnanli/xClaw/issues/326) Part 3b — issue 自带研究范围

---

## 7. Sign-off

| 角色 | 姓名 | 状态 | 时间 |
|---|---|---|---|
| Architect | nally | ⏳ pending | — |
| Author | GitHub Copilot agent | ✅ drafted | 2026-05-08 |
