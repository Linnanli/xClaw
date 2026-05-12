# ADR-144: Linux WritableRoot kernel enforcement (Wave-C1c — Phase 0 spike)

- **Status**: Proposed (Phase 0 spike — research / planning only, **no Rust code in this PR**)
- **Date**: 2026-05-12
- **Authors**: GitHub Copilot (drafted under agent task), reviewed by [pending — human]
- **Tracks**: [#438](https://github.com/Linnanli/xClaw/issues/438)
- **Closes (Phase 0 only)**: epic [#380](https://github.com/Linnanli/xClaw/issues/380) Wave-C1c "Phase 0 spike + ADR draft" deliverable
- **References**:
  - ADR-129 §1.3 — verbatim port 红线
  - ADR-135 §3 — sandboxing crate adoption / Wave-C 拆分
  - ADR-141 §1.1 — three-platform support matrix + fail-closed default
  - ADR-142 — WritableRoot kernel enforcement (cross-platform charter)
  - ADR-143 — macOS residual sinking (sibling Wave-C decision)
  - codex [`linux-sandbox`](https://github.com/openai/codex/tree/main/codex-rs/linux-sandbox) crate — verbatim source

---

## 1. Context

Epic [#380](https://github.com/Linnanli/xClaw/issues/380) Wave-C1c 是 ADR-141 三平台矩阵最后一个 🔴 行：

> Linux `read_only_subpaths` **内核层** 兜底（当前 [`crates/dasclaw_sandbox/src/linux/mod.rs`](../../../crates/dasclaw_sandbox/src/linux/mod.rs) 只做 seccomp 网络阻断，文件系统隔离为 0；与 Windows DACL DENY / macOS seatbelt 不对称）。

W2.3 落 Linux backend 时 [`crates/dasclaw_sandbox/src/linux/mod.rs:1-30`](../../../crates/dasclaw_sandbox/src/linux/mod.rs) 头部 doc 已显式声明：

> - 文件系统限制走 **bubblewrap** (`bwrap.rs` 2,005 LOC)，需要外部 setuid 二进制，**本切片不取**。
> - landlock 路径在 codex 注释里标记为 "legacy/backup"，**本切片不取**。

也就是说 Wave-C1c 的工作是**补齐 W2.3 故意推迟的两项能力**：把 codex `linux-sandbox` crate verbatim 端口进来，让 Linux 路径与 Windows / macOS 同样具备 kernel-level FS 兜底。

本 ADR 是 **Phase 0 spike**：

- 不动任何 Rust 代码；
- 完成 codex `linux-sandbox` 三层验证；
- 决定移植路径（fork / 自研 / verbatim port）；
- 回答 epic #438 列出的 6 个 Open Question；
- 给 Phase 1/2 排出可执行计划。

---

## 2. Three-way verification (per AGENTS.md §"分析工具使用规范")

> 本 ADR 的核心结论是"codex `linux-sandbox` crate 是 Wave-C1c 唯一 ADR-129-compliant 来源"，按 AGENTS.md 必须三层验证。

### 2.1 Level 1 — semantic / surface

codex `codex-rs/linux-sandbox` crate 公共面（[`lib.rs`](../../../codex-cli-main/codex-rs/linux-sandbox/src/lib.rs)）：

```rust
#[cfg(target_os = "linux")]
pub fn run_main() -> ! { linux_run_main::run_main(); }
```

唯一公共入口是 `run_main()`，外部以 **binary `codex-linux-sandbox`** 或 **arg0 dispatch**（`codex-exec` 主二进制检测 arg0 后跳转）使用。crate 内部模块 (`bwrap` / `landlock` / `launcher` / `linux_run_main` / `proxy_routing` / `vendored_bwrap`) 全部 `pub(crate)`。

设计 charter：**"先尝试外部 `bwrap`，缺失则回退到 vendored bubblewrap C 源码（build.rs 编入二进制）；landlock 仅作 legacy fallback"**（[`linux-sandbox/README.md` lines 1-50](../../../codex-cli-main/codex-rs/linux-sandbox/README.md)）。

### 2.2 Level 2 — symbol / structural

crate 规模：

| 文件 | LOC | 用途 |
|---|---|---|
| `bwrap.rs` | 2,005 | bubblewrap argv builder + ripgrep glob 展开 |
| `proxy_routing.rs` | 797 | loopback proxy 路由 |
| `linux_run_main.rs` | 750 | main + inner-exec 重启 + arg0 dispatch |
| `linux_run_main_tests.rs` | 535 | 测试 |
| `landlock.rs` | 343 | legacy landlock fallback（gated by `features.use_legacy_landlock`） |
| `launcher.rs` | 226 | 子进程 launcher |
| `vendored_bwrap.rs` | 78 | FFI 进 build.rs 编入的 `bwrap_main` 符号 |
| `lib.rs` / `main.rs` | 33 | entrypoints |
| **总计** | **4,767** | |

依赖（[`Cargo.toml:18-28`](../../../codex-cli-main/codex-rs/linux-sandbox/Cargo.toml)）：

| codex crate | 我方对应 | 状态 |
|---|---|---|
| `codex-protocol` | `dasclaw_protocol` | ✅ verbatim 已落 (Wave-A1) |
| `codex-sandboxing` | `dasclaw_sandboxing` | ✅ verbatim 已落 (Wave-B1) |
| `codex-utils-absolute-path` | `dasclaw_absolute_path` | ✅ 已落 |
| `landlock` / `seccompiler` / `libc` / `globset` | 外部 crate | ✅ workspace 可直拉 |

`build.rs` 编译 `codex-rs/vendor/bubblewrap/` C 源码并 expose `bwrap_main` C 符号；需要 `pkg-config libcap-dev` build dep。

### 2.3 Level 3 — literal

```text
$ grep -rn "setuid" codex-cli-main/codex-rs/linux-sandbox/src/
landlock.rs:57:  // setuid privilege elevation. Many `bwrap` deployments rely on setuid, so …
linux_run_main.rs:66: /// This exists so we can run bubblewrap first (which may rely on setuid)

$ grep -rnE "unshare-user|--unshare-pid" codex-cli-main/codex-rs/linux-sandbox/src/bwrap.rs
bwrap.rs:153: "--unshare-user".to_string(),
bwrap.rs:154: "--unshare-pid".to_string(),

$ ls codex-cli-main/codex-rs/vendor/bubblewrap/COPYING
# GNU LIBRARY GENERAL PUBLIC LICENSE Version 2, June 1991
# (LGPL v2 — see OQ-C1c-7)

$ ls codex-cli-main/codex-rs/vendor/
BUILD.bazel  bubblewrap

$ grep -rn "kernel.unprivileged_userns_clone\|kernel.apparmor_restrict_unprivileged_userns" \
    codex-cli-main/.github/workflows/
rust-ci-full.yml:649: # Required for bubblewrap to work on Linux CI runners.
rust-ci-full.yml:651:   sudo sysctl -w kernel.unprivileged_userns_clone=1
rust-ci-full.yml:654:   sudo sysctl -w kernel.apparmor_restrict_unprivileged_userns=0
```

### 2.4 三层验证小结

| 维度 | 证据 | 结论 |
|---|---|---|
| L1 | crate 公共面 = `run_main()` 单一入口 + arg0 dispatch | 端口边界清晰，只需顶出一个 binary |
| L2 | 4,767 LOC / 6 ports + vendored bubblewrap，三个上游 codex crate 依赖**全部已落** | 端口无未解依赖；workspace 集成成本可控 |
| L3 | 上游显式注释"may rely on setuid"；CI sysctl 需求公开记录；vendored bwrap C 源码就在 `vendor/bubblewrap/` | privilege model 与运行需求事实可考证 |

**结论：codex `linux-sandbox` crate 是 Wave-C1c 唯一 ADR-129-compliant 来源**；自研 bwrap / landlock 不在选项内（在 verbatim-locked workspace 中等于补丁式代码）。

---

## 3. Options considered

| 选项 | 描述 | 评估 |
|---|---|---|
| **A — Verbatim port full `codex-linux-sandbox` crate** | 新建 `crates/dasclaw_sandbox_linux/`，byte-for-byte 端口 `lib.rs` / `bwrap.rs` / `landlock.rs` / `launcher.rs` / `linux_run_main.rs` / `proxy_routing.rs` / `vendored_bwrap.rs` + `build.rs` + `vendor/bubblewrap/`，仅把 `codex_*::` 改为 `dasclaw_*::` 顶层路径，加 drift guard 脚本 | ✅ 与 `dasclaw_sandbox_windows` 范式对称；零自写业务代码；ADR-129 §1.3 兼容；上游升级路径已就位（drift guard） |
| **B — Self-roll bwrap + landlock from scratch** | 仅取 codex 的 design ideas，自己写一份 Rust-only bubblewrap 替代品 | ❌ **直接违反 ADR-129 §1.3**；4,767 LOC × verbatim-locked workspace = 极高 drift 风险；安全敏感代码自写无 upstream 互查 |
| **C — Use `landlock` crate only (skip bwrap)** | 只走 codex 的 legacy landlock 路径，跳过 bubblewrap 整 2 KLOC | ❌ 上游已把 landlock 标 "legacy/backup"，nesting carveout 不被支持；与 ADR-141 §1.1 三平台对称目标不符（macOS seatbelt 支持嵌套，Windows DACL 支持嵌套） |
| **D — Defer Wave-C1c indefinitely** | 维持 Linux 路径"seccomp 网络阻断 + 应用层 cap-std"现状，正式作废 ADR-141 §1.1 Linux 行 | ❌ ADR-141 §6 OQ-1 fail-closed 默认要求 Linux 路径有 kernel FS 兜底；放弃即承认三平台不对称，回归到 #28 之前的状态 |
| **E — Fork codex repo as git submodule** | 把 codex 整 repo submodule 进来，直接复用 `codex-linux-sandbox` 不端口 | ❌ submodule 与现有 `dasclaw_sandbox_windows` / `dasclaw_sandboxing` 端口模式不对称（其它 sandbox crate 都走 verbatim port + drift guard，submodule 引入第二套上游同步机制）；submodule pin 漂移检测要求开发者保持 submodule init / update，CI 复杂度提高；当前 codex-cli-main 已是工作区子目录（非 submodule），无强行换形态的必要 |

### 3.1 为什么 A 是唯一可接受路径

Wave-B1 `dasclaw_sandboxing` 和 Wave-C1a/b `dasclaw_sandbox_windows` 已为本 epic 建立标准范式：**"上游 codex crate ⇄ `dasclaw_*` 同名 crate ⇄ drift guard 脚本"**。`dasclaw_sandbox_linux` 是该范式的最后一块拼图。任何偏离 A 的选项要么破红线（B/C），要么破设计模式（E），要么破 ADR-141 charter（D）。

---

## 4. Decision

**Option A — verbatim port `codex-linux-sandbox` crate as `dasclaw_sandbox_linux`.**

Phase 0 本 PR 仅落本 ADR，不写任何 Rust 代码。Phase 1 / 2 计划见 §5.2 / §5.3。

### 4.1 决策结果

1. 新 crate `crates/dasclaw_sandbox_linux/`，byte-for-byte 端口 codex `codex-rs/linux-sandbox/`（含 `build.rs` 和 `vendor/bubblewrap/`），仅做 `codex_*` → `dasclaw_*` 顶层路径替换。
2. `crates/dasclaw_sandbox/src/linux/mod.rs` 改为 adapter：保留现有 seccomp 网络阻断作 cross-cut，调 `dasclaw_sandbox_linux::run_main` 完成 FS 隔离，模式与 `crates/dasclaw_sandbox/src/windows/mod.rs` 对称。
3. 新增 drift guard `scripts/check_codex_linux_sandbox_drift.py`，模式参照 `scripts/check_codex_sandboxing_drift.py` / `check_codex_sandbox_windows_drift.py`。
4. 接 `dasclaw_sandbox::check_enterprise_gate` Step 3 Linux arm：完成 kernel 接线后由 `EnterpriseGateOutcome::SoftMode(SoftModeReason::LinuxNoKernelReadOnlySubpaths)` 改为直接 `Allow`，删除 `SoftModeReason::LinuxNoKernelReadOnlySubpaths` 变体和 `decide_carveout_outcome` Linux 分支（参照 PR #437 删 `WindowsNoKernelReadOnlySubpaths` 的模板）。
5. CI 在 Linux job 注入 codex 已用的 sysctl + apt 步骤（[详见 §5.4](#54-ci-feasibility)）。

### 4.2 红线影响

- ADR-129 §1.3 verbatim 红线：本决策**强化**红线（端口走与 `dasclaw_sandboxing` / `dasclaw_sandbox_windows` 完全一致的范式）。
- ADR-141 §6 OQ-1 fail-closed 默认：保留——Phase 0 / Phase 1 实施期间，`check_enterprise_gate` 对 Linux 仍返回 `SoftMode(LinuxNoKernelReadOnlySubpaths)`，由企业策略决定是 Deny 还是 Allow with soft-mode audit；Phase 2 落地后直接 `Allow`。
- ADR-114 命名迁移：本 PR 是 **类A**（无新增 `.ironclaw` / `IRONCLAW_BASE_DIR` 字面量）。
- `scripts/check_no_panics.py`：codex `linux-sandbox` 内有少量 `panic!`（如 `vendored_bwrap.rs:23`）—— Phase 1 端口时按 PR #426 / W2.3 模板加 `# verbatim-port-allow` annotation（参照 `dasclaw_sandbox_windows` 现状）。

---

## 5. Consequences

### 5.1 Phase 0（本 PR）立即生效

- Epic #380 Wave-C1c 行状态由 `🔴 未启动 / Phase 0 spike pending` 改为 `🟡 ADR-144 Proposed`（spike 完成，待 Phase 1 启动）。
- `crates/dasclaw_sandbox/src/linux/mod.rs:1-30` 头部 doc 在 Phase 1 启动 PR 同步加 "ADR-144 决定 verbatim port" 注脚（本 PR 不动 Rust 源码）。
- Wave-C1c 工作量评估校正：4,767 LOC verbatim + ~200 LOC adapter + ~150 LOC drift guard + CI workflow ≈ **XL 不变**，但可拆为 P1.1（crate 端口 only，零 wiring）、P1.2（adapter wiring + gate flip）、P1.3（drift guard + CI sysctl）三个 sub-PR。

### 5.2 Phase 1 计划（不在本 PR）

| 子任务 | 边界 | 验证 |
|---|---|---|
| P1.1 | 新建 `crates/dasclaw_sandbox_linux/` byte-for-byte 端口 + `vendor/bubblewrap/` 拷入 + `build.rs` 拷入 + `Cargo.toml` workspace 注册 | 主要由 CI `ubuntu-latest` job 完成 `cargo build -p dasclaw_sandbox_linux`；macOS dev 仅做 `cargo check --target x86_64-unknown-linux-gnu` 走 cfg-gate 自测（实际 C 代码不编）；`scripts/check_codex_linux_sandbox_drift.py` 接线 |
| P1.2 | `crates/dasclaw_sandbox/src/linux/mod.rs` adapter wiring：spawn 由 `dasclaw_sandbox_linux::run_main` 接管 FS，保留现 seccomp 网络阻断；`check_enterprise_gate` Step 3 Linux arm flip 为 `Allow`；删 `SoftModeReason::LinuxNoKernelReadOnlySubpaths` + `decide_carveout_outcome` Linux 分支 | 现有 `dasclaw_sandbox` 测试族 + 新增 H1/H2 测试（参照 ADR-112 §5.4） |
| P1.3 | drift guard 脚本 + `.github/workflows/code_style.yml` `failure-check` stanza + Linux CI job 注入 `kernel.unprivileged_userns_clone=1` + `apt install pkg-config libcap-dev`（直抄 [`codex-rs/.github/workflows/rust-ci-full.yml:644-655`](../../../codex-cli-main/.github/workflows/rust-ci-full.yml)） | drift guard self-test；CI green on `ubuntu-latest` |

### 5.3 Phase 2 计划（不在本 PR）

- 在 GHA `ubuntu-latest` 上跑 Linux backend 真机集成测试（child process bwrap 跑、`read_only_subpaths` 真生效断言）。
- 把 Linux 加入 ADR-141 §1.1 矩阵"已落"行；epic #380 Wave-C1c 行改为 🟢。

### 5.4 CI feasibility

GitHub Actions `ubuntu-latest` 已被 codex 在自己 CI 上验证过可用（[`codex-rs/.github/workflows/rust-ci-full.yml:644-655`](../../../codex-cli-main/.github/workflows/rust-ci-full.yml)），所需操作：

```yaml
- name: Enable unprivileged user namespaces (Linux)
  if: runner.os == 'Linux'
  run: |
    sudo sysctl -w kernel.unprivileged_userns_clone=1
    if sudo sysctl -a 2>/dev/null | grep -q '^kernel.apparmor_restrict_unprivileged_userns'; then
      sudo sysctl -w kernel.apparmor_restrict_unprivileged_userns=0
    fi
- name: Install Linux bwrap build dependencies
  if: runner.os == 'Linux'
  run: |
    sudo apt-get update -y
    sudo DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends pkg-config libcap-dev
```

无需 self-hosted runner，无需 privileged container。`apparmor_restrict_unprivileged_userns` sysctl 仅 Ubuntu 24.04+ 存在；codex 的 conditional `grep` 检测兼容更老的 runner image。Phase 1 PR 直接复用此片段。

### 5.5 反例 / 否决记录

- 不要"先做 landlock-only 简化版再补 bwrap"——landlock 不支持 nested carveout，会让 Linux 路径与 macOS / Windows 行为永久不对称，且 Phase 1 收益负值（landlock 代码后续仍需保留作 legacy fallback，但不是主路径）。
- 不要把 `dasclaw_sandbox_linux` 合并进 `dasclaw_sandboxing`——后者是 policy/argv-only verbatim crate，linux-sandbox 是 spawn/exec verbatim crate，charter 不同（与 ADR-143 §2.4 同理）。
- 不要因为 macOS dev 机器不能本地跑 Linux 测试就把 CI gate 放弃——drift guard + GHA Linux job 即可保证回归覆盖（与 Windows 路径同步骤）。

---

## 6. Open questions（对应 epic [#438](https://github.com/Linnanli/xClaw/issues/438) §"Phase 0 Open Questions"，+ 自审新增 OQ-C1c-7）

### OQ-C1c-1：移植路径

**回答**：见 §3 / §4，选 **Option A（verbatim port full crate）**。其它选项均破红线或破范式。

### OQ-C1c-2：setuid / 特权模型

**回答**：codex `codex-linux-sandbox` 二进制本身**不需要 setuid**——其 spawn 模型走 user namespace（`unshare(CLONE_NEWUSER)`）。setuid 仅是发行版 `bwrap` 包的潜在配置选择（codex 注释 [`landlock.rs:57`](../../../codex-cli-main/codex-rs/linux-sandbox/src/landlock.rs)、[`linux_run_main.rs:66`](../../../codex-cli-main/codex-rs/linux-sandbox/src/linux_run_main.rs)）。我们的发行制品（`dasclaw_sandbox_linux` 编出的 binary）**不应**带 setuid bit；运行需求降级为 distro 必须开启 `kernel.unprivileged_userns_clone=1`（Ubuntu 22.04+ 默认开），Ubuntu 24.04+ 额外要求 `kernel.apparmor_restrict_unprivileged_userns=0` 或为我方 binary 配 AppArmor profile。WSL1 不支持（codex 已明文拒绝），WSL2 正常路径。

### OQ-C1c-3：CI runner

**回答**：GHA `ubuntu-latest` 即可（见 §5.4）。无需 self-hosted。Phase 1 PR 直接抄 codex 的 sysctl + apt 片段。

### OQ-C1c-4：wiring 边界

**回答**：与 Windows 路径对称——`dasclaw_sandbox_linux` 持 verbatim core，`crates/dasclaw_sandbox/src/linux/mod.rs` 作 adapter 调用 + 保留现 seccomp 网络阻断作 cross-cut；`check_enterprise_gate` Step 3 Linux arm 在 Phase 1.2 完成后 flip 为 `Allow`，对应删 `SoftModeReason::LinuxNoKernelReadOnlySubpaths` 变体和 `decide_carveout_outcome` Linux 分支（模板 = PR #437）。

### OQ-C1c-5：proxy_routing.rs 端口边界

**回答**：codex `proxy_routing.rs` (797 LOC) 是 loopback proxy 桥接，与 `dasclaw_sandbox::SandboxBackendConfig.proxy_loopback_ports`（已就位）配套。verbatim 端口意味着 codex `proxy_routing` 模块整体进入 `dasclaw_sandbox_linux`。现 [`crates/dasclaw_sandbox/src/linux/mod.rs`](../../../crates/dasclaw_sandbox/src/linux/mod.rs) 自有的 seccomp ProxyRouted 实现是 W2.3 临时实现，Phase 1.2 adapter wiring 时**必须**走 codex `proxy_routing`（不能保留两条 ProxyRouted 路径），W2.3 临时实现作为技术债同步清理。Phase 1.2 PR 描述要列出该 ProxyRouted 替换的回归测试列表。

### OQ-C1c-6：legacy landlock 保留策略

**回答**：codex 上游把 landlock 标 "legacy fallback"，由 `features.use_legacy_landlock` 开关启用。Phase 1 verbatim 端口保留该开关；我方默认值 = `false`（与 codex 默认对齐），enterprise 配置可显式打开。**不要在 Phase 1 删 landlock 代码**——保持 verbatim 完整性，drift guard 才能起作用。

### OQ-C1c-7：vendored bubblewrap LGPL v2 许可证影响（新增）

**问题**：`codex-rs/vendor/bubblewrap/` 是上游 LGPL v2；本 workspace 顶层是 MIT OR Apache-2.0 双许可。vendored C 源码会被 codex `build.rs` **静态编入** 二进制，LGPL §6 对此种链接有重新链接条款要求。

**初步分析**：
- 现状：codex 自己就是 vendored static-link 进 OpenAI 产品；codex 已在自己的 NOTICE / `third_party/` 处理此问题，我们端口时**必须**同步抄它的 NOTICE 文件，并把 `bubblewrap/COPYING` 一同纳入 `crates/dasclaw_sandbox_linux/vendor/bubblewrap/`。
- LGPL §6 通常的合规路径：(a) 发行 bubblewrap 修改后源码 + (b) 提供 "重新链接对象" 或动态链接选项。codex 选 (a)（vendor/ 目录本身已是 modified source 分发），我们随之即可。
- **本 ADR 不解决该法务问题**，仅显式记录；Phase 1.1 PR 必须包含 NOTICE/THIRD_PARTY 文件同步，并由人类评审签字。

**推荐**：Phase 1.1 PR 描述必须列：(1) `crates/dasclaw_sandbox_linux/vendor/bubblewrap/COPYING` 已拷入；(2) 顶层 NOTICE / `third_party/` 更新；(3) `cargo-deny` license allowlist 是否需要追加 LGPL-2.0；(4) 法务签字状态。

**Tracking**：已拆出独立 sub-issue [#440](https://github.com/Linnanli/xClaw/issues/440) — `chore(legal): LGPL v2 compliance for vendored bubblewrap`。**#440 是 Phase 1.1 的硬 blocker**：必须在 Phase 1.1 PR 中同时 close 才允许合入，否则 Phase 1.1 暂停。

---

## 7. Sign-off

- Status: **Proposed**（待 nally 评审；评审通过后改为 **Accepted** 并签 Date / Authors，同步在 epic #380 ROADMAP 表 Wave-C1c 行打 🟡，开 Phase 1.1 子 issue）
- 不属于 `adr-redline`：本 ADR 不改变 fail-closed 策略，仅做移植路径决策；Phase 1 实施 PR 各自再评审。
- Cross-cuts (ADR-114)：类A（本 PR 仅新增本文档，无 `.ironclaw` / `IRONCLAW_BASE_DIR` 字面量）。
