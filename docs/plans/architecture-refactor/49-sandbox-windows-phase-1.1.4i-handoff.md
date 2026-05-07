# Sandbox-Windows Phase 1.1.4i — Wave i 系列交付与剩余文件 handoff

> Tracker: #263 (Phase 1.1.4 parent), #250 (Phase 1.1), #241 ([Epic] Fork)
> Crate: `crates/dasclaw_sandbox_windows`
> Upstream: `openai/codex` `codex-rs/windows-sandbox-rs/` @ commit `6e838a19fa`
> Crate phase doc: `1.1.4i-9`

## 1. 已完成

Phase 1.1.4i 通过「Wave-batched verbatim port」节奏推进，至 Wave i-4 已完成 **30 个文件**（含 `setup_orchestrator.rs` 别名为 `setup`）的 verbatim 端口，零 rework，零红 CI。

### Wave 序列与合并 PR

| Wave | PR | 文件 | LOC |
|---|---|---|---|
| i-1 (per-file) | #294 | `policy.rs` | ~ |
| i-1 (per-file) | #296 | `process.rs` | ~ |
| i-1 (per-file) | #298 | `read_acl_mutex.rs` | ~ |
| i-1 (per-file) | #300 | `acl.rs` | ~ |
| i-1 (per-file) | #302 | `sandbox_users.rs` | ~ |
| i-1 (per-file) | #304 | `ssh_config_dependencies.rs` | ~ |
| **i-2 (batch)** | #306 | `allow.rs` + `audit.rs` + `workspace_acl.rs` | ~ |
| **i-3 (batch)** | #308 | `helper_materialization.rs` + `setup_orchestrator.rs` | ~ |
| **i-4 (batch)** | #310 | `identity.rs` + `spawn_prep.rs` | 646 |

策略切换收益：i-2/i-3/i-4 batch 相比单文件 PR 节奏，节省 ~70% 流水线/CI/review 周期。

### 已注册模块（lib.rs，30 项）

`absolute_path`, `acl(cfg)`, `allow`, `audit(cfg)`, `cap`, `desktop(cfg)`, `dpapi(cfg)`, `env`, `helper_materialization(cfg)`, `hide_users(cfg)`, `identity(cfg)`, `logging`, `path_normalization`, `policy`, `proc_thread_attr(cfg)`, `process(cfg)`, `pty`, `read_acl_mutex(cfg)`, `sandbox_users(cfg)`, `sandbox_utils`, `setup(cfg, #[path]=setup_orchestrator.rs)`, `setup_error(cfg)`, `spawn_prep(cfg)`, `ssh_config_dependencies`, `string_util`, `token(cfg)`, `types`, `winutil(cfg)`, `workspace_acl(cfg)`。

### Crate-root re-exports

- `helper_materialization::resolve_current_exe_for_launch`
- `identity::require_logon_sandbox_creds` / `sandbox_setup_is_complete`
- `setup::{SETUP_VERSION, SandboxSetupRequest, SetupRootOverrides, run_elevated_setup, run_setup_refresh, run_setup_refresh_with_extra_read_roots, sandbox_bin_dir, sandbox_dir, sandbox_secrets_dir}`

## 2. 剩余 3 个文件 — 全部需要设计决策

| 文件 | LOC | 阻塞类型 | Wave 名 |
|---|---|---|---|
| `firewall.rs` | 512 | 重型依赖适配 | i-5 |
| `elevated_impl.rs` + 配套 | 306 + ~1500 | lib.rs 主体重构 | i-6 |
| `setup_main_win.rs` | 899 | lib/bin split | i-7 |

### 2.1 Wave i-5: `firewall.rs` (512 LOC) — 重型 windows-crate 适配

**当前 deps**: `windows-sys = 0.52`（轻量）。

**新增需求**: `windows = 0.58` 重型 COM crate + 特性 `Win32_NetworkManagement_WindowsFirewall` / `Win32_System_Variant` / `Win32_System_Com`。

**风险点**:
- `windows` crate 体积大 + 编译耗时；与现有 `windows-sys` 共存需评估 vendoring/firewall 镜像。
- upstream 标注此文件为 bin-only（不在 `windows_modules!` 宏内），意味着 lib 暴露面需要决策。

**建议**: 单独写 ADR 评估「`windows` vs `windows-sys` 共存策略」+「firewall API 是否对 lib 暴露」，再启动端口。

### 2.2 Wave i-6: `elevated_impl.rs` (306 LOC) — 依赖整个 `elevated/` 子目录

**调研结果**：`elevated_impl.rs` 不止依赖 `ipc_framed`，实际 transitive 依赖：

| 文件 | LOC | 必需性 |
|---|---|---|
| `elevated/ipc_framed.rs` | 192 | 直接 |
| `elevated/runner_client.rs` | 218 | 直接 (`spawn_runner_transport`) |
| `elevated/runner_pipe.rs` | 135 | 间接 |
| `elevated/cwd_junction.rs` | 142 | 间接 |
| `elevated/command_runner_win.rs` | 569 | 间接 |
| `windows_impl` (inline `mod` in upstream lib.rs) | ~250 | 直接 (`CaptureResult`, `run_windows_sandbox_capture`) |

合计 **~1500 LOC + lib.rs 主体重组** — 远超单 Wave 量级。

**风险点**:
- upstream lib.rs 含 inline `mod windows_impl { ... }` 块（约 240+ LOC），verbatim 端口意味着把这一大块也照搬进我方 lib.rs。
- 我方 lib.rs 当前是「模块注册 + 阶段文档 + 非 Windows fallback」三段式，与 upstream 风格已分歧。
- 如果不照搬而是抽出独立 `windows_impl.rs` 文件，就属于「补丁式重构」，违反 verbatim 红线。

**建议**: 写 ADR 决策「inline `windows_impl` 是否照搬 / 抽出 / 缓后」。在决策前不启动该 Wave。

### 2.3 Wave i-7: `setup_main_win.rs` (899 LOC) — bin 入口

upstream 中此文件为 `setup-main-win` 二进制的 entry。我方目前仅以 lib 形式存在。

**风险点**:
- 需 lib/bin split：要么在 `crates/dasclaw_sandbox_windows/` 内新增 `[[bin]]` target，要么走 `desktop-client` / `admin-backend` 调用路径。
- 与 `helper_materialization::resolve_current_exe_for_launch` 的运行期协议必须一致（已有 lib 端，bin 端要对得上）。

**建议**: 与 `desktop-client` / `admin-backend` 当前 launcher 设计对照，写一节执行计划再启动。

## 3. 接力建议（next session）

### 3.1 必读

1. `AGENTS.md` — 中文规约总入口（六步本地管线、PR 模板、verbatim port 红线、跨会话轮换原则）。
2. 本文件（49-sandbox-windows-phase-1.1.4i-handoff.md）。
3. `crates/dasclaw_sandbox_windows/src/lib.rs` 的 module doc（`Crate status (Phase 1.1.4i-9)`）。
4. tracker issue #263 / #250 当前状态。

### 3.2 启动顺序

下一个 milestone（Phase 1.1.4j）建议：

1. **先决议 ADR-129「sandbox-windows: `windows` crate 适配策略」**（covers Wave i-5 + 同步评估对 i-6 inline `windows_impl` 的影响）。
2. **再决议 ADR-130「sandbox-windows: lib/bin split for setup_main_win」**（covers Wave i-7）。
3. ADR 落地后按 i-5 → i-6 → i-7 顺序 verbatim port，每 Wave 一个 PR。

### 3.3 不建议立即启动

直接拉 Wave i-6 单独 port `ipc_framed.rs`：会得到一段「孤儿模块」无消费者，违反 implementationDiscipline 的「Don't add features beyond what was asked」。

## 4. 验证清单（接班 agent 可直接复用）

```bash
# 1. crate 单点 check（< 30 sec）
cargo check -p dasclaw_sandbox_windows --tests

# 2. crate 单点 nextest（45 tests, ~7 sec）
cargo nextest run -p dasclaw_sandbox_windows

# 3. fmt + 红线脚本
cargo fmt --all
python3.12 scripts/check_no_panics.py --base origin/xClaw
python3 scripts/check_no_new_ironclaw_literal.py --base origin/xClaw

# 4. crate 单点 clippy（不跑 workspace）
cargo clippy --no-deps -p dasclaw_sandbox_windows --all-targets -- -D warnings
```

完整 `cargo build` / workspace clippy / Windows-only integration 由 CI 兜底。

## 5. 已规避的陷阱（教训库）

1. **重名 issue**：曾出现 #249/#250、#259/#260、#262/#263 三对 API 重试副作用 dup（已于本会话清理）；新建 tracking issue 后用 `gh issue list --search "<title> in:title"` 查重。
2. **`#[allow(dead_code)]` 的 cross-platform 副作用**：`ssh_config_dependencies.rs` 的消费者 `setup` 是 `cfg(windows)`；非 Windows 上未消费。正确写法：`#[cfg_attr(not(windows), allow(dead_code))]`。
3. **`setup_orchestrator.rs` 的 `pub(crate)` 友邻可见性**：`identity.rs` 通过 `crate::setup::xxx` 访问 `pub(crate) fn gather_read_roots` 等，编译可通过；不要为此添加 `pub`。
4. **upstream `bin-only` 文件不在 `windows_modules!` 宏内**：在决定端口前先 `grep` 上游 lib.rs，避免误估为 lib 模块。
5. **`#[path = "..."]` 模块别名**：`setup_orchestrator.rs` 通过 `#[cfg(windows)] #[path = "setup_orchestrator.rs"] pub mod setup;` 镜像 upstream，不要重命名文件本身。

## 6. 标签与流程契约

- PR 标题前缀：`feat(sandbox-windows):`
- PR body 必含：背景/目标 / 改动范围 / 非目标 / 验证 / `Closes #N`
- Issue 标签：`area:sandbox`,`area:platform`,`phase:P1`（**不要**用 `phase-1.1.4i`/`port`/`sandbox-windows`，它们不存在）
- 分支命名：`feat/sandbox-windows-wave-iX[-batch]`
- 分支 base：始终 `xClaw`（不要 stack）
- Squash merge + `--delete-branch`
