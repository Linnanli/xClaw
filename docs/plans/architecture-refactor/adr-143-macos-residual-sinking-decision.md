# ADR-143: macOS residual capability sinking decision (Wave-C2)

- **Status**: Proposed (decision: **Option D — keep adapter in `dasclaw_sandbox`**, no code move)
- **Date**: 2026-05-12
- **Authors**: GitHub Copilot (drafted under agent task), reviewed by [pending — human]
- **Tracks**: [#432](https://github.com/Linnanli/xClaw/issues/432)
- **Closes**: Epic [#380](https://github.com/Linnanli/xClaw/issues/380) Wave-C2 row (resolved as "no-op by design")
- **References**:
  - ADR-129 §1.3 — verbatim port 红线
  - ADR-135 §3 — sandboxing crate adoption / Wave-C 拆分
  - ADR-141 §1.1 — three-platform support matrix
  - Epic [#380](https://github.com/Linnanli/xClaw/issues/380) — Sandbox mainline finalisation

---

## 1. Context

Epic [#380](https://github.com/Linnanli/xClaw/issues/380) ROADMAP REVISED 表格里 Wave-C2 行写明：

> `crates/dasclaw_sandbox/src/macos/mod.rs` 残余下沉（env_clear / setrlimit / memorystatus / spawn lifecycle 看是否能再下沉到 `dasclaw_sandboxing`）。

四项能力当前的归属与历史出处：

| # | 能力 | 当前位置 | 来源 |
|---|---|---|---|
| 1 | `Command::env_clear()` + 选择性 env 透传（防止 `AWS_*` / `OPENAI_API_KEY` 等泄漏） | `crates/dasclaw_sandbox/src/macos/mod.rs:106-117` | W2.2c P1 修 |
| 2 | `pre_exec` 调用 `crate::rlimit::apply_in_pre_exec` 套 CPU / FD / NPROC | `crates/dasclaw_sandbox/src/macos/mod.rs:124-134` | W3.3-2 |
| 3 | post-spawn `memorystatus::set_memory_limit(child.id() …)` 内存上限（macOS `RLIMIT_AS` 返回 `EINVAL` 的绕行） | `crates/dasclaw_sandbox/src/macos/mod.rs:136-145` | W3.3-3b，docs/plans/architecture-refactor/46 |
| 4 | `spawn() + wait_with_output()` 生命周期（为了拿到 child pid 再调 memorystatus，替代 `output()`） | `crates/dasclaw_sandbox/src/macos/mod.rs:139-148` | W3.3-3b 配套 |

ADR-129 §1.3 把 `dasclaw_sandboxing`（codex `sandboxing` crate 的整 crate verbatim port）锁定为 verbatim 边界：禁止在该 crate 内自写 codex 没有的业务逻辑。本 ADR 决策这四项要不要移出 `dasclaw_sandbox/src/macos/mod.rs`、移到哪里、移动后红线如何处理。

---

## 2. Three-way verification (per AGENTS.md §"分析工具使用规范")

> 本 ADR 的核心是一个否定性结论："codex `sandboxing` crate 没有 env_clear / setrlimit / memorystatus / spawn lifecycle 这些能力"，按 AGENTS.md 必须三层验证。

### 2.1 Level 1 — semantic / surface

`crates/dasclaw_sandboxing/src/manager.rs` 暴露的公共 API 是 `SandboxManager::{new, select_initial, transform}`：

```rust
pub fn select_initial(...) -> SandboxType { ... }
pub fn transform(req: SandboxTransformRequest<'_>) -> ... { ... }
```

`SandboxExecRequest` / `SandboxTransformRequest` 都是 **argv builder 输入** —— `manager.rs` 仅生成 `Vec<String>` argv 给消费侧的 `Command`，自身**不执行进程**。这是 codex 上游 `sandboxing` crate 的设计选择：policy-only / argv-only，进程生命周期留给 `core/exec.rs`。

### 2.2 Level 2 — symbol / structural

`SandboxManager::transform` 返回 argv 字符串数组（参考 `crates/dasclaw_sandbox/src/macos/mod.rs:59-86` 中 `build_seatbelt_args` 调用 `create_seatbelt_command_args` 后只拿回 `Vec<String>`，再由 `dasclaw_sandbox` 自己 `Command::new(...).args(&args).env_clear().pre_exec(...).spawn()`）。换言之 sandboxing crate 在调用图上是 **leaf transform layer**，不出现在 child-process 生命周期路径上。

### 2.3 Level 3 — literal

```text
$ grep -rnE 'env_clear|setrlimit|memorystatus|pre_exec|wait_with_output' codex-cli-main/codex-rs/sandboxing/
(无输出)

$ grep -rn 'memorystatus' codex-cli-main/codex-rs/
(无输出 —— codex 全仓零引用)

$ grep -rnE 'env_clear|setrlimit' codex-cli-main/codex-rs/core/src/exec*.rs
codex-cli-main/codex-rs/core/src/exec_env.rs:11:/// `env_clear()` to ensure no unintended variables are leaked to the spawned …
```

结论：**codex `sandboxing` crate 不持有这 4 项能力中的任何一项**。codex 将 env 处理放在 `core/exec_env.rs`，进程 spawn 放在 `core/exec.rs`；memorystatus / setrlimit 在 codex 全仓零引用。

### 2.4 三层验证小结

| 维度 | 证据 | 结论 |
|---|---|---|
| L1 | sandboxing crate API = policy/argv-only | crate charter 不收 spawn lifecycle |
| L2 | `SandboxExecRequest` 是 argv 输入而非执行上下文 | sandboxing 不在 spawn 调用图上 |
| L3 | 0 matches for env_clear/setrlimit/memorystatus/pre_exec/wait_with_output in codex `sandboxing/`；memorystatus 在 codex 全仓 0 matches | 上游不仅没实现，也无路径准备实现 |

四项能力对 `dasclaw_sandboxing` verbatim 边界而言是**外部** capability，把它们塞进去等于在 verbatim-locked crate 内自写 codex 没有的代码，明确违反 ADR-129 §1.3。

---

## 3. Options considered

| 选项 | 描述 | 评估 |
|---|---|---|
| **A — Sink to `dasclaw_sandboxing`** | 把 4 项搬进 `dasclaw_sandboxing::manager` 或新建 `dasclaw_sandboxing::exec_lifecycle` | ❌ **违反 ADR-129 §1.3**（在 verbatim-locked crate 内自写 upstream 没有的代码）；同时打破 codex sandboxing crate "policy/argv-only" charter |
| **B — Upstream proposal to codex** | 在 codex `sandboxing` crate 加 env_clear / setrlimit / memorystatus 支持，让 dasclaw 后续 verbatim 拉回来 | ❌ off-charter（codex 显式把 env / spawn 放在 `core`，不是 `sandboxing`）；memorystatus 是 macOS-specific 且 codex 全仓 0 引用，上游接受概率极低；即便接受，时序远远晚于本 epic 收尾 |
| **C — Move to `dasclaw_exec`** | 把 4 项搬到调用方 `dasclaw_exec` | ⚠️ 部分可行但失去复用：rlimit/memorystatus 是 macOS 平台 glue，不该污染 `dasclaw_exec` 跨平台代码；env_clear 与 `req.command.get_envs()` 透传配对紧密，搬走会破坏 sandbox backend trait 契约 |
| **D — Keep adapter in `dasclaw_sandbox` (current state)** | 维持现状：`dasclaw_sandboxing` 出 argv，`dasclaw_sandbox/src/macos/mod.rs` 套 env_clear + pre_exec + post-spawn memorystatus | ✅ 与 Windows 路径对称（`dasclaw_sandbox/src/windows/mod.rs` 也是 \"verbatim core + 平台 adapter\"，DACL DENY 接线就在 adapter 层）；零 verbatim 红线风险；零代码移动；ownership 已稳定 |
| **E — New `dasclaw_macos_runtime` crate** | 抽出新 crate 专放 macOS runtime glue | ❌ over-engineering：仅 4 个函数 + ~40 LOC，`mod.rs` 总 416 LOC，crate 边界成本高于收益；与 ADR-002 三层架构（execution/policy/protocol）正交，不在分层蓝图内 |

### 3.1 为什么 D 与 Windows / macOS 已有架构对称

ADR-141 §1.1 Windows 层级表的最后一行（"Kernel-layer `read_only_subpaths` enforcement"）正是 **verbatim core (`dasclaw_sandbox_windows` + codex sandboxing 公开 API) + 平台 adapter (`crates/dasclaw_sandbox/src/windows/mod.rs`)** 的范式：DACL DENY 转换在 adapter 层，不在 verbatim crate 里。同样的归责，macOS 的 env_clear / pre_exec / memorystatus / spawn 应该留在 adapter（`crates/dasclaw_sandbox/src/macos/mod.rs`）。

---

## 4. Decision

**Option D — keep adapter in `dasclaw_sandbox`.**

Wave-C2（epic #380）的"看是否能下沉" 答案是：**不下沉。当前位置就是正确位置。**

### 4.1 决策结果

1. `crates/dasclaw_sandbox/src/macos/mod.rs` 保留 env_clear / pre_exec setrlimit / post-spawn memorystatus / spawn+wait_with_output lifecycle 四项 ownership，**不移动任何代码**。
2. `dasclaw_sandboxing` 保留 verbatim 边界，不引入 codex 上游没有的 spawn lifecycle 代码。
3. Epic #380 Wave-C2 行结论改为 `🟢 决策完成（ADR-143 Option D — 维持现状）`；不再列入"剩余任务"。
4. 未来涉及 macOS env / rlimit / memorystatus 的新功能默认仍落在 `dasclaw_sandbox/src/macos/`，不要尝试推到 `dasclaw_sandboxing`。

### 4.2 红线影响

- ADR-129 §1.3 verbatim 红线：本决策**强化**红线（拒绝把外部 capability 塞进 verbatim crate）。
- `scripts/check_codex_sandboxing_drift.py`：无影响（四项符号本来就不在 upstream，对 drift guard 无新输入）。
- ADR-114 命名迁移：本 PR 是 **类A**（无新增 `.ironclaw` / `IRONCLAW_BASE_DIR` 字面量）。

---

## 5. Consequences

### 5.1 立即生效

- Epic #380 Wave-C2 行状态更新（本 PR 同步改 epic body）。
- 后续涉及 macOS sandbox env / rlimit 的 PR 默认目标文件锁定为 `crates/dasclaw_sandbox/src/macos/mod.rs` 及其 `memorystatus.rs` / `crate::rlimit` 子模块，不需要再走 \"是否下沉\" 评估循环。

### 5.2 不立即生效但需要注意

- 如果未来 codex 上游主动把 env_clear / setrlimit 等加入 `sandboxing` crate（low likelihood per L3 evidence），本 ADR 应触发 superseding ADR 重审。
- 如果出现第二个非 macOS 的 \"adapter-owned spawn lifecycle\" 平台（例如 FreeBSD），且与 macOS 路径出现 ≥30% 代码重复，再考虑抽 `dasclaw_sandbox_runtime` 公共 crate（不是 `dasclaw_sandboxing`）。

### 5.3 反例 / 否决记录

- 不要把 `Sandbox` trait 改为返回 `Vec<String>` argv + 让 `dasclaw_exec` 统一 spawn —— 这会破坏 Windows 路径已就位的 \"sandbox 自己 spawn 自己的 launcher 子进程\" 设计（参考 `crates/dasclaw_sandbox/src/windows/mod.rs` 调 `run_windows_sandbox_capture_*`）。
- 不要为这 4 项能力新建 \"facade\" 模块再单独 re-export —— 直接在 `mod.rs` 内就近持有更清晰；ownership 已通过模块 doc 头部声明（`mod.rs:14-21`）。

---

## 6. Open questions

- **OQ-1**: 是否同步在 `crates/dasclaw_sandbox/src/macos/mod.rs` 头部 doc 添加 \"ADR-143 决定不下沉\" 注脚？→ **推荐：是**，但本 PR 不动 Rust 源码（与 \"0 行代码改动\" 边界一致）；留到 \"epic #380 closeout PR\" 一起做。
- **OQ-2**: 是否需要 closeout epic #380？→ Wave-C2 决策后，epic #380 剩余 Wave-C1c（Linux 内核 WritableRoot）仍 🔴，不能 closeout，仅刷状态。
- **OQ-3**: Linux backend 是否有对称的 \"残余下沉\" 决策需要？→ 不在本 ADR 范围；Linux 路径要等 Wave-C1c ADR（fork codex-linux-sandbox vs 自研 bwrap+landlock）先定，再决定残余分布。

---

## 7. Sign-off

- Status: **Proposed**（待 nally 评审；评审通过后改为 **Accepted** 并签 Date / Authors）
- 不属于 `adr-redline`：本 ADR 不改变 `#28` 等 fail-closed 策略，仅做归属决策。
