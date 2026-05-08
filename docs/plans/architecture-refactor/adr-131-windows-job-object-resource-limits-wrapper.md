# ADR-131: Windows Job Object resource-limits wrapper (side-by-side)

- **Status**: 🟢 **Accepted + B1/B2/B3 落地**（2026-05-08 nally 在 mcp-feedback-enhanced 签字；B1 PR #329 / B2 PR #328 / B3 PR #333；B4 本 PR doc-sync）
- **Date**: 2026-05-08
- **Approver**: nally
- **Authors**: GitHub Copilot agent
- **Tracker**: [#34](https://github.com/Linnanli/xClaw/issues/34) — Windows process-level resource limits via Job Objects
- **Epic 联动**: [#324](https://github.com/Linnanli/xClaw/issues/324) — sandbox 主线补强（与本 ADR 互不阻塞）
- **Related**:
  - [ADR-129](adr-129-sandbox-windows-windows-crate-adoption.md) §1.3 — sandbox-windows 端口必须 verbatim，禁止补丁式改写（**红线**）
  - [ADR-130](adr-130-sandbox-windows-lib-bin-split.md) — `dasclaw_sandbox_windows` lib + 2 bin 拆分
  - [ADR-45](45-resource-limits-adr.md) §"Axis 3 — Windows (future)" — W3.3 显式延后 Windows resource limits
  - [Phase 1.2 完成报告](50-sandbox-windows-phase-1.2-completion-and-roadmap.md) §"#34 verbatim 红线分析"

---

## 1. Context

### 1.1 触发条件

`crates/dasclaw_sandbox/src/lib.rs` 的 `ResourceLimits` struct 已落地（W3.3 / PR #30 / #32 / #33），Linux 走 cgroup v2、macOS 走 `memorystatus_control`，但 Windows 字段在 doc-comment 明确标注 *silently ignored on Windows / future*。Phase 1.2（PR #321）落地了 Windows adapter 但**未启用**资源限制。

### 1.2 上游事实（已三层验证）

`codex-cli-main/codex-rs/windows-sandbox-rs/src/elevated/command_runner_win.rs:103-130`（verbatim 镜像在 `crates/dasclaw_sandbox_windows/src/elevated/command_runner_win.rs:103-130`）创建的 Job Object **只设了** `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`：

```rust
limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
```

**没有** `JOB_OBJECT_LIMIT_PROCESS_MEMORY` / `JOB_OBJECT_LIMIT_ACTIVE_PROCESS` / `JOB_OBJECT_LIMIT_JOB_MEMORY`。这是 codex 上游也存在的安全缺口（不只是 dasclaw 落后）。

### 1.3 ADR-129 红线

[ADR-129](adr-129-sandbox-windows-windows-crate-adoption.md) §1.3 明文：

> sandbox-windows 端口必须 **verbatim**，禁止补丁式改写。新增功能（含资源限制）必须在 dasclaw 侧另起 wrapper / adapter，不在 verbatim crate 内打补丁。

直接编辑 `command_runner_win.rs` 加 limits = 违反红线。本 ADR 设计的所有方案都遵守"verbatim crate 不动"。

### 1.4 Windows Job Object 嵌套语义（Win 8+ / Server 2012+）

- 一个进程同时属于至多 1 个 *直接* job，但可属于一条 *嵌套* job 链
- 嵌套 job 的 limits 取**最严**（OS 自动 enforce 链上所有 job 的限制并集）
- 子进程默认 `inherit` 父进程的 job 关联（除非 spawn 时设了 `JOB_OBJECT_LIMIT_BREAKAWAY_OK` 且 `CREATE_BREAKAWAY_FROM_JOB`）
- 因此：**只要 verbatim crate 不显式 breakaway，外层 wrapper 在父进程 job 加 memory/process limits，孙进程会受限**

这条语义是本 ADR 三个候选方案的物理基础。

---

## 2. 决策（待签字）

**采用方案 B**：在 `dasclaw_sandbox` adapter 新增 binary `dasclaw-sandbox-resource-launcher.exe`，作为外层 Job Object 容器，间接调用 `dasclaw_sandbox_windows::run_windows_sandbox_capture`。

详细方案对比见 §3。

---

## 3. 候选方案对比

### Option A — adapter 进程内套 outer job

**做法**：`WindowsRestrictedTokenSandbox::execute` 在调用 `run_windows_sandbox_capture` 前，给 *当前进程* 创建 outer Job Object 设置 memory/process limits，然后调 lib API。

**优点**：
- 实现简单（~80 LOC FFI + 1 处接入）
- 无新 binary、无 IPC、无序列化协议
- effort: S（半天）

**致命缺点**：
- adapter 进程通常是 `dasclaw-server` / `desktop-client`（长期运行，多任务）。把它整个塞进 memory limit 会导致 server 进程自身被 kill（OOM 政策影响调用方而非沙箱目标）
- 多个并发 sandbox 任务无法各自独立 limit（所有任务共用 server 进程的 outer job）
- 隔离失败：limit 是给 sandbox 任务的，不是给 host 的

**结论**：❌ 拒绝。语义错位。

### Option B — 新 launcher binary（推荐）

**做法**：在 `dasclaw_sandbox` crate 新增 binary（不在 `dasclaw_sandbox_windows`，避免触红线）：

```toml
# crates/dasclaw_sandbox/Cargo.toml
[[bin]]
name = "dasclaw-sandbox-resource-launcher"
path = "src/bin/resource_launcher_win.rs"
required-features = []  # cfg(target_os="windows") gate inside
```

Binary 流程：
1. 从 stdin 读 JSON (`SandboxExecRequest` + `ResourceLimits` + `policy_json`)
2. `CreateJobObjectW` + `SetInformationJobObject` 设置：
   - `JOB_OBJECT_LIMIT_PROCESS_MEMORY` ← `max_memory_bytes`
   - `JOB_OBJECT_LIMIT_ACTIVE_PROCESS` ← `max_processes`
   - `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` (RAII 卫生)
3. `AssignProcessToJobObject(outer_job, GetCurrentProcess())`
4. 调 `dasclaw_sandbox_windows::run_windows_sandbox_capture(...)`
5. upstream 内部再创建 inner job（仅 `KILL_ON_JOB_CLOSE`），是 outer 的 nested child
6. arbitrary command 在 inner job 中跑，受 outer job 的 memory/process limits 自动 enforce
7. 把 `CaptureResult` 写到 stdout（JSON）退出

Adapter 改动：
- `WindowsRestrictedTokenSandbox::execute` 改为 spawn launcher 子进程（用 `std::process::Command`），传 stdin / 收 stdout
- 不直接调 lib API（保留 lib API 作为 launcher 的 dependency）

**优点**：
- ✅ 隔离边界清晰：launcher 进程是一次性的，被 kill 只影响一次 sandbox 调用
- ✅ 多并发 sandbox 任务各自独立 limit（每次 spawn 一个 launcher）
- ✅ verbatim crate 完全不动（合规 ADR-129）
- ✅ 失败语义干净：launcher 退出码 / stderr 直接反映 limit 触发情况
- ✅ launcher 自身可被 unit-test（FFI + IPC 协议测）

**缺点**：
- 多一次进程 fork 开销（~10ms）
- 新 IPC 协议表面（JSON over stdin/stdout）；需版本管理
- effort: M（1-3 day）

**风险**：low-med（FFI 对 + IPC 协议；都是 well-trodden ground）

### Option C — pre-exec helper via `CREATE_SUSPENDED`

**做法**：adapter 用 `CreateProcessW` with `CREATE_SUSPENDED` 创建一个 helper（其实就是 verbatim crate 的入口），suspended 状态下 `AssignProcessToJobObject(outer_job, helper.hProcess)`，然后 `ResumeThread`。

**优点**：
- 不需要新 binary（复用现有 entry）
- 无 IPC，参数照常用 stdin/argv

**致命缺点**：
- adapter 必须直接 `CreateProcessW` 而不是 `Command::spawn`（要拿 PROCESS_INFORMATION）
- adapter 复制了 verbatim crate 的 spawn 逻辑（lpCommandLine 拼接、env block、cwd）→ 实际上是把 lib API 的"easy mode"绕开了
- 与 lib API 的 `run_windows_sandbox_capture` (高层 API) 不兼容；要么改 lib（违反 ADR-129），要么 adapter 重新实现 capture (大量重复代码)

**结论**：❌ 拒绝。低层 spawn 路径与 lib API 抽象冲突。

---

## 4. 推荐方案 B 的实施切片

> 实施在 PR-2 进行，本 ADR (PR-1) 不含代码改动。

### Slice B1 — launcher binary 骨架（effort: S）— ✅ Done（PR #329）
- `crates/dasclaw_sandbox/src/bin/resource_launcher_win.rs`
- `Cargo.toml` `[[bin]]` 入口（cfg-gated 到 windows）
- 单元测试：JSON IPC 协议 round-trip
- **不**实际调 lib API（dry-run mode）

### Slice B2 — Job Object FFI（effort: S）— ✅ Done（PR #328）
- `crates/dasclaw_sandbox/src/windows/job_object.rs`（新文件）
- FFI: `CreateJobObjectW` + `SetInformationJobObject` + `AssignProcessToJobObject`
- RAII guard：`Drop` 自动 `CloseHandle`
- 单元测试：在 cfg(windows) 下创建-attach-drop 一次

### Slice B3 — adapter 接入（effort: S）— ✅ Done（PR #333）
- `WindowsRestrictedTokenSandbox::execute` 改为 spawn launcher
- IPC：JSON stdin/stdout
- 集成测试（cfg(windows) only）：spawn 一个 child，set memory cap，断言子进程在分配超过 cap 时被终止

### Slice B4 — doc 同步（effort: S）— 🟡 In progress（本 PR）
- ADR-45 §"Axis 3 — Windows" section 改为 `(implemented via outer Job Object launcher; ADR-131)`
- doc 51 §3 capability matrix：Windows 列 process memory / active process 改 ✅；§5.1 标 RESOLVED
- `crates/dasclaw_sandbox/src/lib.rs` `ResourceLimits` 字段 doc-comment 删 *future / silently ignored* 标记（仅 max_memory_bytes；`max_cpu_secs` / `max_open_files` 仍保留 silent no-op 标记）

每个 slice 独立 PR。Slice B1 + B2 可并行；B3 依赖前两个；B4 在 B3 后。

---

## 5. 非目标

- ❌ CPU time limits (`JOB_OBJECT_LIMIT_PROCESS_TIME`)：单位与 `RLIMIT_CPU` 语义不同，独立 ADR
- ❌ Open-file-handle limits：Windows 无对应概念
- ❌ 修改 verbatim crate 内的 inner job（ADR-129 红线）
- ❌ AppContainer / 完全替换 Restricted Token：与 #324 sub-task 无关

---

## 6. 验证（PR-2 实施时跑）

按 [`AGENTS.md`](../../AGENTS.md) 6 步本地管线，每个 slice：

```bash
cargo check -p dasclaw_sandbox --all-targets
cargo nextest run -p dasclaw_sandbox
cargo fmt --all
python3.12 scripts/check_no_panics.py --base origin/xClaw
python3 scripts/check_no_new_ironclaw_literal.py --base origin/xClaw
cargo clippy --no-deps -p dasclaw_sandbox --all-targets -- -D warnings
```

Windows 集成测试在 `.github/workflows/windows-ci.yml` 的 windows-latest runner 跑（PR #322 已扩 paths 覆盖 `crates/dasclaw_sandbox/**`）。

---

## 7. Open questions — 已签字回答

| # | 问题 | 决议 | 理由 |
|---|---|---|---|
| 1 | launcher binary 名字 | **`dasclaw-sandbox-resource-launcher.exe`** | 命名清晰、与现有 `dasclaw-sandbox-setup` / `dasclaw-command-runner` 风格一致 |
| 2 | 失败语义 | **新增 `SandboxError::WindowsLauncherFailed { detail }`** | 与现有 `WindowsSetupPending` 语义不同（前者是"setup 没做"，后者是"setup 已完成但本次执行的 wrapper 出问题"）。合并会误导用户去重跑 setup。独立 variant 让错误处理与 UI 提示能 match 不同分支 |
| 3 | Slice 顺序 | **B1 + B2 并行起步** | 两个 slice 互不依赖（B1 是 binary 骨架 + IPC，B2 是 FFI 单测），并行可缩短时长；B3 等两者都 land 后再开 |
| 4 | binary build gating | **方案 (a) — 无条件 build + 非 Windows stub main** | Cargo `[[bin]]` 不支持 `[target.'cfg(...)'.bin]` 直接 gate；`required-features` 方案虽然 Linux/macOS 不产 stub binary，但增加 CI 复杂度与"feature 忘开"误用风险。桌面端项目 CI 简单可靠更重要 |

非 Windows 平台 stub 实施模板：

```rust
// crates/dasclaw_sandbox/src/bin/resource_launcher_win.rs
#[cfg(target_os = "windows")]
fn main() -> anyhow::Result<()> {
    // 真实逻辑：读 stdin JSON → 创 outer Job Object → 设 limits → AssignProcessToJobObject → 调 lib API → 写 stdout
}

#[cfg(not(target_os = "windows"))]
fn main() {
    eprintln!("dasclaw-sandbox-resource-launcher is Windows-only");
    std::process::exit(1);
}
```

---

## 8. 后续工作

- 本 ADR 通过后开 PR-2 stack 实施 §4 四个 slice（每 slice 一个 PR）
- ADR-45 更新由 Slice B4 PR 顺带完成
- doc 51 / doc 50 由 Slice B4 PR 顺带更新
- #324 sub-task（process-hardening / WritableRoot kernel / net_proxy）与本 ADR 互不阻塞，可并行
