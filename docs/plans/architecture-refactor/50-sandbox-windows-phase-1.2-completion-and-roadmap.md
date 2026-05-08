# 50 — Windows Sandbox: Phase 1.2 完成 + 后续路径分析

> **Status**: Informational follow-up after PR #321 (Phase 1.2)
> **Date**: 2025-11
> **Scope**: Windows sandbox roadmap 状态盘点 / issue 依赖图 / CI 触发器调整
> **Source PRs**: #319 (Phase 1.1.4j) → #321 (Phase 1.2) → 本文档（PR #322）

## 1. 当前状态

| Phase | 内容 | Issue | PR | 状态 |
|-------|------|-------|-----|------|
| 1.1.4j | `dasclaw_sandbox_windows` verbatim port (codex `6e838a19fa`) | #250 | #319 | ✅ 已合并 |
| 1.2 | `dasclaw_sandbox` adapter + `OsExecutor` wire-up | #320 | #321 | ✅ 已合并 |
| 1.3 | hardening (private desktop / DPAPI / extra-deny paths) | (TBD) | — | ⏳ 待规划 |
| 2.x | enterprise admin shell wiring | #28 / #94 / #95 | — | 🔒 阻塞于 P0-A |

## 2. CI 触发器调整

`.github/workflows/windows-ci.yml` 的 `push.paths` filter 在 PR #319 设计时只覆盖
`crates/dasclaw_sandbox_windows/**` 与 `vendor/codex-windows-sandbox/**`。
PR #321 引入了 `crates/dasclaw_sandbox/src/windows/mod.rs` 这层 adapter，
但路径不在 filter 内，导致 PR #321 自身的 Windows job 被自动 skip。

**本 PR 修复**：把 `crates/dasclaw_sandbox/**` 加入 `paths`，以后任何 dasclaw
跨平台 sandbox 抽象的改动都自动触发 Windows CI 回归保护。

为什么不在 PR #321 内一起改？— 当时未识别此盲点，PR #321 已经合并；按
"非补丁式" 原则，路径修复独立成 PR 比 squash 进 #321 更干净。

## 3. 待办 issue 状态盘点

### 3.1 #250 — Phase 1.1 tracker

**结论**：可关闭。Wave i-1 ~ i-4 全部 verbatim port 已落地（PR #319 完成报告
`docs/plans/architecture-refactor/49-sandbox-windows-phase-1.1.4j-completion.md`），
本 issue 是过程跟踪 tracker，使命已完成。本 PR 通过 `Closes #250` 关闭。

### 3.2 #34 — Windows process-level resource limits via Job Objects (W5)

**当前阻塞**：`SetInformationJobObject` 调用点位于
`crates/dasclaw_sandbox_windows/src/elevated/command_runner_win.rs:121`，
当前只设了 `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`（lifecycle），没有 memory /
active-process caps。

要在该处注入 `JOBOBJECT_EXTENDED_LIMIT_INFORMATION.JobMemoryLimit` 和
`JOBOBJECT_BASIC_LIMIT_INFORMATION.ActiveProcessLimit`，必须修改 verbatim
port 文件 — 与 [ADR-129 §1.3](./adr-129-sandbox-windows-windows-crate-adoption.md)
的 verbatim 红线冲突（"sandbox-windows 端口必须 verbatim，禁止补丁式改写"）。

**两条合规路径**（任选其一推进）：

1. **Side-by-side wrapper**：在 `dasclaw_sandbox_windows` 新增
   `resource_limits.rs` 模块，提供 RAII guard `JobLimitGuard`，由 dasclaw
   adapter 在 spawn 之后取 child PID → `OpenProcess` → 创建第二个 Job →
   `AssignProcessToJobObject`。技术可行但与上游 Job 重叠，需 ADR 评估
   "双 Job assignment" 的语义（Windows 文档允许，但 KILL_ON_JOB_CLOSE
   交互需测）。
2. **上游 PR**：在 codex `windows-sandbox-rs` 加可选 `ResourceLimits`
   字段，等合入后 cherry-pick 回来。最稳但节奏受上游约束。

**建议**：等 Phase 1.3 hardening ADR 一起评估上述两条路径，本期不动。
issue #34 保持 open，添加 link 到本文档作为后续上下文。

### 3.3 #91 — Windows enterprise sandbox/resource-limit support plan

**依赖**：#28 (P0-A enterprise shell fail-closed contract)。#91 acceptance
明确"Windows enterprise behavior is explicit and test-covered"，但
fail-closed 的 contract 表面在 #28 / #94 那条线上还未敲定。

**建议**：保持 open；在 P0-A 落地之前先把"Windows 上 sandbox 不可用时
**必须** fail-closed"作为一行测试加入 `dasclaw_sandbox` 已有的
`require_on_unsupported_returns_unavailable` 同款套路 — 但它本质上不是
issue #91 的全部 acceptance，只是 trip-wire。本期不混入。

## 4. 非目标 (What's NOT in this PR)

- ❌ 不实施 #34 的 Job Object 资源限制（受 verbatim 红线挡住，需先 ADR）
- ❌ 不关闭 #34 / #91（实质工作未完成）
- ❌ 不动任何 verbatim port 文件
- ❌ 不引入新的 sandbox 行为变更

## 5. 验证

唯一代码改动是 yml `paths` 数组的扩展。生效路径：
本 PR 自身就是路径**外**的改动（`.github/workflows/` 已在原 paths 内，
所以本 PR 仍会触发 Windows CI），同时验证扩展项 — 后续触动
`crates/dasclaw_sandbox/**` 的 PR 会自动跑 Windows CI。

## 6. Cross-cuts

- ADR-129 / 130 — 未触动 verbatim port，红线安全
- ADR-114 — 无新 `.ironclaw` literal
- 治理：本 PR 关闭 1 个 tracker issue (#250)，其余 issue 保持 open 并附
  阻塞分析，便于后续会话直接接力。
