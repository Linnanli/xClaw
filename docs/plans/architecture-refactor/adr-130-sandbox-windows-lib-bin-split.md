# ADR-130: sandbox-windows lib/bin split for setup_main_win

- **Status**: � **Accepted**（2026-05-07 nally 在 mcp-feedback-enhanced 签字）
- **Date**: 2026-05-07
- **Approver**: nally
- **Authors**: GitHub Copilot agent (sandbox-windows port worker)
- **Tracker**: [#263](https://github.com/Linnanli/xClaw/issues/263) — Phase 1.1.4 parent
- **Epic**: [#241](https://github.com/Linnanli/xClaw/issues/241) — fork codex-windows-sandbox
- **Hand-off**: [`49-sandbox-windows-phase-1.1.4i-handoff.md`](49-sandbox-windows-phase-1.1.4i-handoff.md) §2.2 / §2.3
- **Related**:
  - [ADR-121](adr-121-p0a-sandbox-activation-decision.md) — P0-A 沙箱默认激活
  - [ADR-129](adr-129-sandbox-windows-windows-crate-adoption.md) — `windows` crate 引入授权（前置）

## 1. Context

### 1.1 触发条件

Hand-off §2.2 / §2.3 指出，剩余 Wave i-6 / i-7 不是简单的"verbatim 单文件 port"——它们改变 `crates/dasclaw_sandbox_windows` 的**目标产物形态**。

### 1.2 上游事实（已三层验证）

`codex-cli-main/codex-rs/windows-sandbox-rs/Cargo.toml` 同时产出 **lib + 2 个 bin**：

```toml
[lib]
name = "codex_windows_sandbox"
path = "src/lib.rs"

[[bin]]
name = "codex-windows-sandbox-setup"
path = "src/bin/setup_main.rs"

[[bin]]
name = "codex-command-runner"
path = "src/bin/command_runner.rs"
```

#### Bin 1: `codex-windows-sandbox-setup` (Wave i-7)

`src/bin/setup_main.rs` 仅 12 行，作为入口 stub：

```rust
#[path = "../setup_main_win.rs"]
mod win;
#[cfg(target_os = "windows")]
fn main() -> anyhow::Result<()> { win::main() }
```

`setup_main_win.rs`（899 LOC）声明 `mod firewall;`（即 firewall.rs 是该 bin 的子模块），**调用** lib 暴露的 ~28 个 `pub` 符号（`add_deny_write_ace` / `canonicalize_path` / `convert_string_sid_to_sid` / `extract_setup_failure` / `hide_newly_created_users` / `load_or_create_cap_sids` / `sandbox_bin_dir` / `to_wide` 等）。

#### Bin 2: `codex-command-runner` (Wave i-6 子项)

`src/bin/command_runner.rs` 同样 12 行 stub，挂 `elevated/command_runner_win.rs`（569 LOC）。这是 hand-off §2.2 列出的 elevated 子目录的一员。

#### Lib 主体的 inline `windows_impl`

上游 `lib.rs` 含 inline `mod windows_impl { ... }` 块（约 240+ LOC），导出 `CaptureResult` / `run_windows_sandbox_capture`。这是 lib 的一部分（不是 bin），消费 `elevated_impl.rs / runner_client / runner_pipe / cwd_junction`。

### 1.3 我方现状

我方 `crates/dasclaw_sandbox_windows/Cargo.toml` 当前**只有 `[lib]`**，无 `[[bin]]`。`lib.rs` 是「模块注册 + 阶段文档 + 非 Windows fallback」三段式，**未 inline `windows_impl`**。

### 1.4 问题分解

剩余 3 个文件的真实依赖链（hand-off §2.2 已调研）：

| 文件 | LOC | 归属 | 依赖 |
|---|---|---|---|
| `firewall.rs` | 512 | bin `setup` 的子模块 | 仅由 `setup_main_win.rs` 消费 |
| `setup_main_win.rs` | 899 | bin `setup` 主体 | 消费 lib 的 ~28 pub 符号；声明 `mod firewall;` |
| `elevated_impl.rs` | 306 | lib（inline `windows_impl` 的实际实现） | 拉取整个 elevated/ 子目录 |
| `elevated/ipc_framed.rs` | 192 | lib | 直接 |
| `elevated/runner_client.rs` | 218 | lib | 直接 |
| `elevated/runner_pipe.rs` | 135 | lib | 间接 |
| `elevated/cwd_junction.rs` | 142 | lib | 间接 |
| `elevated/command_runner_win.rs` | 569 | bin `command-runner` 主体 | 间接 |

### 1.5 候选方案

| 选项 | 描述 | 评估 |
|---|---|---|
| **A**（推荐） | 完全对齐上游：新增 `[[bin]] dasclaw-sandbox-setup` 与 `[[bin]] dasclaw-command-runner`，文件路径与上游 1:1 镜像；lib.rs 引入 inline `windows_impl` 与 elevated 子目录注册 | verbatim port 红线零让步；与 ADR-121 W4 档位 C 实施路径一致；后续 desktop-client launcher 调用 bin 时无适配层 |
| **B** | 把所有 bin 内容合并到 lib，新增公开 API `run_setup_main()` / `run_command_runner()`；不产 bin | 严重违反 verbatim 红线（上游 bin 用 `fn main()` panics 且依赖 process exit code 协议）；desktop-client 必须改路径调用方式 |
| **C** | bin split 缓后到 P2；先把 elevated 子目录搬到 lib，setup_main_win/firewall 暂不 port | 留下 `helper_materialization::resolve_current_exe_for_launch` 的下游消费悬空（运行期协议要求 setup bin 存在）；不可接受 |

## 2. Decision

**采用方案 A：完全对齐上游 lib + 2 bin 形态。**

执行顺序（Phase 1.1.4j）：

### 2.1 Wave i-5（先做，仅 lib，bin-only 文件）

`firewall.rs` 端口为「未注册的 src 文件」，**不**写入 lib.rs，**不**新增 bin。仅在文件末尾打 `#![cfg(target_os = "windows")]` gate；该文件在 Wave i-7 由 setup bin 通过 `mod firewall;` 引入。

> 与 hand-off §2.1 描述一致。该 PR 不引入 lib API 变更。

### 2.2 Wave i-6（lib 主体重组）

分两步，**两个 PR**：

**i-6a**：port 整个 elevated/ 子目录（5 个文件，1257 LOC），在 lib.rs 注册：

```rust
#[cfg(target_os = "windows")]
#[path = "elevated/ipc_framed.rs"]
pub(crate) mod ipc_framed;
#[cfg(target_os = "windows")]
#[path = "elevated/runner_pipe.rs"]
mod runner_pipe;
#[cfg(target_os = "windows")]
#[path = "elevated/runner_client.rs"]
mod runner_client;
#[cfg(target_os = "windows")]
#[path = "elevated/cwd_junction.rs"]
mod cwd_junction;
// command_runner_win 不在 lib 注册，由 bin 引用
```

**i-6b**：port `elevated_impl.rs` + 上游 inline `windows_impl { ... }` 块。**严格 verbatim**：上游 lib.rs 怎么 inline，我方 lib.rs 也 inline；不允许"抽出独立 windows_impl.rs"——那是补丁式重构。

> 与 hand-off §5 教训库第 4 条一致：发现"上游 inline" 时必须照搬，不重构。

### 2.3 Wave i-7（bin split，最终成型）

**两个 PR**（分别对应两个 bin）：

**i-7a**：新增 `[[bin]] dasclaw-sandbox-setup`：

```toml
[[bin]]
name = "dasclaw-sandbox-setup"
path = "src/bin/setup_main.rs"
```

新增文件 `src/bin/setup_main.rs`（12 行 stub，verbatim 上游）+ port `setup_main_win.rs`（899 LOC verbatim，仅替换 crate root `codex_windows_sandbox::*` → `dasclaw_sandbox_windows::*`）。

**i-7b**：新增 `[[bin]] dasclaw-command-runner` + port `elevated/command_runner_win.rs`（569 LOC verbatim）。

### 2.4 命名约束（ADR-114 一致）

- bin 名称：`dasclaw-sandbox-setup` / `dasclaw-command-runner`（**不**用 `codex-*` 上游名；上游名是产品路径标识，verbatim 不包含产品命名空间）
- 文件路径：`src/bin/setup_main.rs` / `src/bin/command_runner.rs`（与上游 1:1）
- desktop-client 调用方需在 ADR-121 W4 档位 C 实施时对齐这两个 bin 名称

## 3. Consequences

### 3.1 正向

- 与上游 1:1 形态对齐，rebase 上游零摩擦
- desktop-client launcher 拥有明确产物名（`dasclaw-sandbox-setup.exe` / `dasclaw-command-runner.exe`）
- `helper_materialization::resolve_current_exe_for_launch` 的下游消费有真实文件可指
- Wave i-5 / i-6 / i-7 串行可执行，每 Wave 都是单 PR 可 review 量级（除 i-6/i-7 各拆 a/b 两 PR）

### 3.2 负向

- crate 产物从 1 个 lib 变成 1 lib + 2 bin，CI build matrix 多 2 个 target（仅 Windows runner 有影响）
- Wave i-7 必须等 Wave i-6 lib 暴露面完整后才能动；流水线变长

### 3.3 缓解

- Wave i-6a 单独可合，i-6b 紧随其后；i-7a / i-7b 相互无文件重叠，可按 `AGENTS.md` "PR 加速策略 手段 2" 并行开 PR（前提：cat-scan 验证零文件重叠后再并发）

## 4. Validation

每个 Wave PR 必跑：

```bash
cargo check -p dasclaw_sandbox_windows --tests
cargo nextest run -p dasclaw_sandbox_windows
cargo fmt --all
python3.12 scripts/check_no_panics.py --base origin/xClaw
python3 scripts/check_no_new_ironclaw_literal.py --base origin/xClaw
cargo clippy --no-deps -p dasclaw_sandbox_windows --all-targets -- -D warnings
```

i-7 完成后，CI Windows runner 必须能产出 2 个 bin 二进制。完整 `cargo build` 由 CI 兜底。

## 5. Non-goals

- 不涉及 `windows` crate 引入决策（→ ADR-129）。
- 不涉及 desktop-client / admin-backend 如何 launch 这两个 bin（→ ADR-121 W4 档位 C 后续实施 ADR）。
- 不为 bin 添加非 Windows 平台 stub fallback（上游 bin `main()` 在非 Windows 直接 `panic!`，verbatim 即可）。
- 不评估"是否合并 setup + command-runner 为单 bin 多子命令"——上游就是 2 个 bin，verbatim 红线下不重组。

## 6. References

- 上游 [`Cargo.toml`](../../../codex-cli-main/codex-rs/windows-sandbox-rs/Cargo.toml) `[[bin]]` 定义
- 上游 [`src/bin/setup_main.rs`](../../../codex-cli-main/codex-rs/windows-sandbox-rs/src/bin/setup_main.rs)
- 上游 [`src/bin/command_runner.rs`](../../../codex-cli-main/codex-rs/windows-sandbox-rs/src/bin/command_runner.rs)
- 上游 [`src/setup_main_win.rs`](../../../codex-cli-main/codex-rs/windows-sandbox-rs/src/setup_main_win.rs) 头部 28 个 `use codex_windows_sandbox::*` 导入
- [Hand-off §2.2 Wave i-6](49-sandbox-windows-phase-1.1.4i-handoff.md#22-wave-i-6-elevated_implrs-306-loc--依赖整个-elevated-子目录)
- [Hand-off §2.3 Wave i-7](49-sandbox-windows-phase-1.1.4i-handoff.md#23-wave-i-7-setup_main_winrs-899-loc--bin-入口)
