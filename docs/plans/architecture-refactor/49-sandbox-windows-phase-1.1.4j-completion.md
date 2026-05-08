# Sandbox-Windows Phase 1.1.4j — Wave i 完整收官报告

> Tracker: #263 (Phase 1.1.4 parent, **CLOSED**), #250 (Phase 1.1), #241 ([Epic] Fork)
> Crate: `crates/dasclaw_sandbox_windows`
> Upstream: `openai/codex` `codex-rs/windows-sandbox-rs/` @ commit `6e838a19fa`
> 前置文档: [`49-sandbox-windows-phase-1.1.4i-handoff.md`](49-sandbox-windows-phase-1.1.4i-handoff.md)

## 1. 摘要

Phase 1.1.4j 承接 1.1.4i 收尾的 3 个剩余文件 + 配套子目录，按 **ADR-129 + ADR-130 → Wave i-5 → i-6a → i-6b → i-7a → i-7b** 的串行节奏全部完成。至此，`dasclaw_sandbox_windows` crate 已与上游 `windows-sandbox-rs/` 在源码层面达成 1:1 镜像（差异仅限 `codex_*` → `dasclaw_*` 重命名 + bin 名重命名 + SPDX 头注释）。

零 rework，零红 CI，issue #263 已自动关闭。

## 2. Wave 序列与合并 PR

| Wave | PR | 文件 | 性质 |
|---|---|---|---|
| ADR | #313 | `adr-129-sandbox-windows-windows-crate-adoption.md` + `adr-130-sandbox-windows-lib-bin-split.md` | 决策 |
| **i-5** | #314 | `firewall.rs` (512 LOC) | 引入 `windows = 0.58` 与 `windows-sys = 0.52` 共存 |
| **i-6a** | #315 | `elevated/cwd_junction.rs` + `ipc_framed.rs` + `runner_pipe.rs` + `runner_client.rs` | elevated 子模块叶子 |
| **i-6b** | #316 | `elevated_impl.rs` + inline `mod windows_impl` / `mod stub` | elevated lib 主体 |
| **i-7a** | #317 | `setup_main_win.rs` + `bin/sandbox_setup.rs` + `[[bin]] dasclaw-sandbox-setup` | bin 1/2 |
| **i-7b** | #318 | `elevated/command_runner_win.rs` + `bin/command_runner.rs` + `[[bin]] dasclaw-command-runner` | bin 2/2 |

合计 **6 个 PR / 5 个 wave，约 +3000 LOC verbatim port + 2 个 ADR**。

## 3. 关键设计决策（ADR）

### 3.1 ADR-129 — `windows` crate 与 `windows-sys` 共存

- `windows-sys = 0.52`：轻量 FFI，已用于 acl / token / process 等核心路径
- `windows = 0.58`：重型 COM crate，仅 firewall.rs 需要（`Win32_NetworkManagement_WindowsFirewall` / `Win32_System_Variant` / `Win32_System_Com`）
- **共存策略**：两者 features 在 `Cargo.toml` 并列声明，编译产物体积可接受（firewall 路径仅 elevated 模式触发）
- 与 codex 上游策略对齐

### 3.2 ADR-130 — lib + 2 bin 拆分

- 1 个 lib：`dasclaw_sandbox_windows`（被 desktop-client 主进程 link）
- 2 个 bin：
  - `dasclaw-sandbox-setup`（对应上游 `codex-sandbox-setup`，elevated setup 路径）
  - `dasclaw-command-runner`（对应上游 `codex-command-runner`，elevated 子进程路径）
- bin 通过 `#[path = "../elevated/xxx.rs"]` 直接 include 实现源体，与 lib 双重编译。**与上游字节级一致**，不引入 `pub` 暴露面变化。

## 4. 最终模块清单（lib.rs）

### 4.1 模块（按 wave 分组）

| Wave 来源 | 模块 |
|---|---|
| 1.1.4i (i-1 ~ i-4) | `absolute_path`, `acl`, `allow`, `audit`, `cap`, `desktop`, `dpapi`, `env`, `helper_materialization`, `hide_users`, `identity`, `logging`, `path_normalization`, `policy`, `proc_thread_attr`, `process`, `pty`, `read_acl_mutex`, `sandbox_users`, `sandbox_utils`, `setup` (alias `setup_orchestrator`), `setup_error`, `spawn_prep`, `ssh_config_dependencies`, `string_util`, `token`, `types`, `winutil`, `workspace_acl` |
| **1.1.4j i-5** | `firewall` |
| **1.1.4j i-6a** | `elevated::{cwd_junction, ipc_framed, runner_pipe, runner_client}` |
| **1.1.4j i-6b** | `elevated_impl` (含 inline `mod windows_impl` / `mod stub`) |
| **1.1.4j i-7a** | `elevated::setup_main_win` |
| **1.1.4j i-7b** | `elevated::command_runner_win` |

### 4.2 Bin targets

| Bin | 路径 | 上游对应 |
|---|---|---|
| `dasclaw-sandbox-setup` | `src/bin/sandbox_setup.rs` | `codex-sandbox-setup` |
| `dasclaw-command-runner` | `src/bin/command_runner.rs` | `codex-command-runner` |

### 4.3 Crate-root re-exports（汇总）

完整 1:1 镜像上游 `lib.rs` 的 `pub use` 列表，覆盖：
- helper_materialization / identity / setup（1.1.4i）
- elevated::{ipc_framed::*, runner_pipe::*, runner_client::*}（i-6a）
- elevated_impl 主导出 + windows_impl / stub 内部类型（i-6b）
- acl / desktop / hide_users / ipc_framed / policy / process / token 系（i-7b 补完）

## 5. 验证

每个 wave PR 均通过统一 6 步本地 gate + 14 SUCCESS CI：

```bash
cargo check -p dasclaw_sandbox_windows --all-targets
cargo nextest run -p dasclaw_sandbox_windows           # 45/45 passed (i-7b)
cargo fmt --all
python3.12 scripts/check_no_panics.py --base origin/xClaw
python3   scripts/check_no_new_ironclaw_literal.py --base origin/xClaw
cargo clippy --no-deps -p dasclaw_sandbox_windows --all-targets -- -D warnings
```

CI 端关键 jobs（每 PR）：Tests (default) · Clippy (default) · Code Style · ADR-114 grep guard · No panics in production code · closes-link · Regression test enforcement · cargo-deny · Formatting · claw-code readonly guard · Windows CI Success · add-and-transition · scope · Run Tests。

Windows runtime 类 jobs（Windows Build / Windows Tests / Heavy Integration / Telegram / WASM / Benchmark / Docker）按 PR-stage policy 在 PR 阶段 SKIP，由 main push 后的 `windows-ci.yml` 与各专项 workflow 兜底。

## 6. ADR-114 合规

全部 6 个 PR 均为「类A」——未新增 `.ironclaw` / `IRONCLAW_BASE_DIR` 字面量，grep guard 全程通过。

## 7. 节奏复盘

| 节奏指标 | 1.1.4i | 1.1.4j |
|---|---|---|
| 文件数 | 30 | 8 + 2 ADR + 2 bin |
| PR 数 | 9 (含 batch) | 6 |
| 单 PR 平均周期（含 CI） | ~30 min | ~25 min |
| Rework 次数 | 0 | 0 |
| 红 CI 次数 | 0 | 0 |

**1.1.4j 关键加速点**：
- ADR 前置（#313）一次性钉死 windows-crate 共存与 lib/bin 拆分两个核心决策，避免 wave 内部反复决议
- i-6 拆 a/b 两个 PR：先消化叶子模块（cwd_junction / ipc_framed / runner_pipe / runner_client），再处理依赖它们的主 lib（elevated_impl + inline windows_impl/stub），降低每个 PR 的 review 复杂度
- i-7 拆 a/b 两个 PR：每个 bin 独立 PR，配套源体 + bin 入口 + Cargo.toml `[[bin]]` 段一次提交，单一职责

## 8. 与上游 codex 的差异面

| 类别 | 差异 |
|---|---|
| crate 名 | `codex_windows_sandbox` → `dasclaw_sandbox_windows` |
| bin 名 | `codex-sandbox-setup` → `dasclaw-sandbox-setup` / `codex-command-runner` → `dasclaw-command-runner` |
| 文件头 | 每个 port 文件首行加 SPDX + `Derived from openai/codex commit 6e838a19fa` provenance 注释 |
| `panic!` 文案 | bin 在 non-Windows 下的 `panic!` 文案使用 dasclaw bin 名 + `// safety:` 注释（`scripts/check_no_panics.py` 要求） |
| 业务逻辑 | **零差异** |

## 9. 后续工作（不在本 phase 内）

### 9.1 Phase 1.1.4k — 集成测试与 Windows CI 实跑

- crate 当前仅有 unit tests（45 项），未跑 sandbox 完整 e2e
- 需切到 Windows runner 环境验证：
  - `dasclaw-sandbox-setup` elevated 路径
  - `dasclaw-command-runner` ConPTY/pipe IPC
  - firewall 规则注入 / token 派生 / ACL 隔离
- 建议独立 issue + 独立 ADR（若需调整 windows-ci.yml 触发条件）

### 9.2 Phase 1.1.5 — 接入 desktop-client 主进程

- `dasclaw_sandbox_windows` 已可被 lib link，但 desktop-client 主进程尚未实际调用
- 涉及 sandbox policy 注入、bin 路径解析（`resolve_current_exe_for_launch`）、setup 触发时机等集成点

### 9.3 Phase 1.2 — Linux 路径

- 与本 windows port 平行的 codex `linux-sandbox-rs/` 移植，独立 phase

## 10. 资产链接

- ADR：[`adr-129-sandbox-windows-windows-crate-adoption.md`](adr-129-sandbox-windows-windows-crate-adoption.md) · [`adr-130-sandbox-windows-lib-bin-split.md`](adr-130-sandbox-windows-lib-bin-split.md)
- PR：[#313](https://github.com/Linnanli/xClaw/pull/313) · [#314](https://github.com/Linnanli/xClaw/pull/314) · [#315](https://github.com/Linnanli/xClaw/pull/315) · [#316](https://github.com/Linnanli/xClaw/pull/316) · [#317](https://github.com/Linnanli/xClaw/pull/317) · [#318](https://github.com/Linnanli/xClaw/pull/318)
- Tracker issue：[#263](https://github.com/Linnanli/xClaw/issues/263) (CLOSED)
- 上游基准：openai/codex commit `6e838a19fa` `codex-rs/windows-sandbox-rs/`

---

**Phase 1.1.4j 收官时间**：2026-04-30
**总投入 PR**：6（含 ADR 1 个 + 实现 5 个）
**总移植代码**：~3000 LOC verbatim port
**结论**：`dasclaw_sandbox_windows` crate 源码层面与 codex 上游 1:1 对齐，可作为后续 1.1.5 主进程接入与 1.1.4k 集成测试的稳定基线。
