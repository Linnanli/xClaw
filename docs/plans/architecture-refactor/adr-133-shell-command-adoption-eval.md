# ADR-133: codex `shell-command` adoption evaluation (research-only)

- **Status**: 🟡 **Decision: adopt-with-adapter** (research-only ADR per [#326](https://github.com/Linnanli/xClaw/issues/326) Part 3a; implementation is **out of scope** for this PR)
- **Date**: 2026-05-08
- **Approver**: pending nally sign-off
- **Authors**: GitHub Copilot agent
- **Tracker**: [#326](https://github.com/Linnanli/xClaw/issues/326) Part 3a — `shell-command` 评估研究 (B-6 + C-5)
- **Related**:
  - [ADR-129](adr-129-sandbox-windows-windows-crate-adoption.md) §1.3 — verbatim port red line
  - [ADR-132](adr-132-execpolicy-starlark-port-plan.md) — execpolicy port 计划（同一 verbatim 模式）
  - [doc 32 §W4-§W5](32-execution-plan.md) — 落地 wave
  - codex 上游：`codex-cli-main/codex-rs/shell-command/`（5 files + `command_safety/`，3,348 LOC）
  - dasclaw 现状：[`crates/dasclaw_bash_validation/src/lib.rs`](../../../crates/dasclaw_bash_validation/src/lib.rs)（980 LOC）

---

## 1. Context

### 1.1 上游事实（已三层验证）

`codex-cli-main/codex-rs/shell-command/` 是一个独立 crate，来源验证：`semantic_search "shell-command parse_command"` → `vscode_listCodeUsages` on `parse_command::parse_command` → `rg "use codex_shell_command"`。

| 文件 | LOC | 角色 |
|---|---|---|
| `lib.rs` | 11 | 公共出口 (`bash`, `parse_command`, `powershell`, `is_safe_command`) |
| `parse_command.rs` | 2,526 | **核心** — 解析任意 shell 命令成结构化 `ParsedCommand`（用于 UI 显示模型在跑什么）|
| `bash.rs` | ~400 | bash AST 解析（`extract_bash_command`, `try_parse_shell`）|
| `powershell.rs` | 189 | PowerShell 解析（`extract_powershell_command`）|
| `shell_detect.rs` | 32 | shell 自动识别（`/bin/bash` vs `pwsh.exe`）|
| `command_safety/` | ~190 | `is_safe_command` / `is_dangerous_command`（基于已 parsed 命令的安全分类）|

公共 API 真理来源：`codex-cli-main/codex-rs/shell-command/src/lib.rs:1-11`。

### 1.2 dasclaw 现状（已三层验证）

[`crates/dasclaw_bash_validation/`](../../../crates/dasclaw_bash_validation/)，源自 `claw-code/rust/crates/runtime/src/bash_validation.rs`（W4 #72 port），共 1,037 LOC：

| 模块 | 职责 |
|---|---|
| `readOnlyValidation` | 在 read-only 模式下拒绝写命令 |
| `destructiveCommandWarning` | `rm -rf /` 类警告 |
| `modeValidation` | 权限模式（plan / execute）下拒绝 |
| `sedValidation` | sed 表达式验证 |
| `pathValidation` | 可疑路径模式检测 |
| `commandSemantics` | 命令意图分类（read / write / network） |

公共出口：`validate_command(cmd, mode) -> ValidationResult { Allow | Block | Warn }`。

### 1.3 关键观察：scope 不重叠

两个 crate 名字相像，但**职责正交**：

| 维度 | codex `shell-command` | dasclaw `dasclaw_bash_validation` |
|---|---|---|
| **目的** | **解析 / 标签** — 把 `bash -c "git status && echo ok"` 解构成 `[GitStatus, Echo("ok")]` 给 UI 显示 | **门控 / 决策** — 在执行前判断 `rm -rf` 是否要 block |
| **输入** | `&[String]` 命令向量 | `&str` 命令 + `PermissionMode` |
| **输出** | `Vec<ParsedCommand>`（结构化标签）| `ValidationResult { Allow / Block { reason } / Warn { message } }` |
| **跨平台** | bash + PowerShell + shell_detect | 仅 bash |
| **依赖** | `shlex` + `tree-sitter-bash`（间接） | 字符串模式匹配 |
| **被谁调用** | UI 渲染层（显示"模型在跑 git status"）| 执行边界（拒绝 / 放行命令） |

**结论**：两个 crate 不是 duplicate，**互相不能替换**。

---

## 2. Decision

### 2.1 决策：**adopt with adapter**（非合并、非废弃）

把 codex `shell-command` 端口为 `dasclaw_shell_command`（独立 crate），与 `dasclaw_bash_validation` **并存**：

```
crates/
  dasclaw_bash_validation/   ← 保留（claw-code 派生，门控决策）
  dasclaw_shell_command/     ← 新增（codex 派生，解析标签）
```

接入关系：

```
   桌面端 UI                                执行边界（dasclaw_governance）
   ───────                                  ────────────────────────────
   parse_command()  ──→ ParsedCommand[]     validate_command(cmd, mode)
   ↑                                          ↓
   dasclaw_shell_command                    dasclaw_bash_validation
   (codex verbatim)                         (claw-code 派生)
                                              ↓
                                            dasclaw_execpolicy
                                            (Policy::evaluate, ADR-132)
```

`dasclaw_shell_command::parse_command()` 输出**不**作为门控信号，仅作 UI 显示。任何 block / warn 决策走 `dasclaw_bash_validation` + `dasclaw_execpolicy`。

### 2.2 端口模式：verbatim port（套用 ADR-129 §1.3 红线）

| 允许的机械改写 | 禁止 |
|---|---|
| `Cargo.toml name = "codex-shell-command"` → `dasclaw_shell_command` | 重构 `parse_command_impl`（2,526 LOC，注释明写 "DO NOT REVIEW THIS CODE BY HAND"）|
| `use codex_protocol::parse_command::ParsedCommand` → `use dasclaw_protocol::parse_command::ParsedCommand`（如 dasclaw_protocol 已落地，否则保留 codex_protocol path-dep）| 改 `is_safe_command` / `is_dangerous_command` 阈值 |
| `bash.rs` 的 `tree-sitter-bash` 依赖保留 | 把 PowerShell 模块裁掉 — 跨平台是核心特性 |

特别地，`parse_command.rs:23` 上游有显式注释：

> `/// DO NOT REVIEW THIS CODE BY HAND`
> `/// This parsing code is quite complex and not easy to hand-modify.`

这与 ADR-129 §1.3 verbatim 红线天然契合 — 任何"小修小改"都会偏离上游测试基线。

### 2.3 公共 API 范围（冻结）

`dasclaw_shell_command::lib.rs` 出口（与上游 1:1 镜像）：

```rust
pub mod bash;
pub mod parse_command;
pub mod powershell;
mod shell_detect;          // private, like upstream
pub(crate) mod command_safety;

pub use command_safety::is_dangerous_command;
pub use command_safety::is_safe_command;
pub use parse_command::{parse_command, extract_shell_command, shlex_join};
```

调用方（按 doc 32 §W4 wave）：

- `desktop-client/ironclaw/src/ui/` — 消费 `parse_command()` 输出渲染"正在执行：git status"
- `dasclaw_bash_validation` **不**反向依赖 `dasclaw_shell_command`（避免门控耦合解析）

### 2.4 mechanical guard

落地 PR 同时引入 `scripts/check_codex_shell_command_drift.py`，与 ADR-132 §3.3 同款：对 `crates/dasclaw_shell_command/src/{parse_command,bash,powershell,shell_detect}.rs` 与 codex 上游做 hash diff，仅允许 §2.2 表格列出的机械改写。

---

## 3. Consequences

### 3.1 Positive

- **dasclaw 桌面端获得真正的 ParsedCommand 渲染能力** — 当前 ironclaw UI 只显示原始命令字符串，端口完成后能渲染 codex 同款交互（"git status" 而非 `bash -c 'git status && echo ok'`）。
- **跨平台 PowerShell 解析白送** — `dasclaw_bash_validation` 仅 bash，端口后 dasclaw 桌面客户端在 Windows 上得到原生 PowerShell parse 能力（与 ADR-129 sandbox-windows port 互补）。
- **门控 / 显示职责拆开** — 后续 `is_dangerous_command` 想升级阈值不影响 `validate_command` 决策，反之亦然。

### 3.2 Negative

- **+3,348 LOC** 进 dasclaw monorepo，绝大部分是 `parse_command_impl` 大体量解析逻辑。
- **`tree-sitter-bash` 依赖**进入 workspace；当前只有 codex 上游使用，verbatim port 必须 vendor 同版本。
- 与 `dasclaw_bash_validation` 名字相近，需要在 `crates/README.md`（如有）+ doc 32 §W4 明确职责边界，避免后续 contributor 复制粘贴功能到错的 crate。

### 3.3 不做合并 / 替换

- ❌ **不**用 `dasclaw_shell_command::is_safe_command` 替换 `dasclaw_bash_validation::validate_command` —— 两者输出语义完全不同（标签 vs 决策）。
- ❌ **不**把 `dasclaw_bash_validation` 重新建立在 `dasclaw_shell_command::parse_command` 之上 —— 会引入"解析→翻译→决策"三跳依赖，且 claw-code 派生的 6 个验证模块很多基于字符串模式而非 AST，硬切代价远超收益。

---

## 4. Implementation sequence（落地 PR 提交序）

> 与 ADR-132 同样：单 PR 内的提交序，不拆 PR。

1. **C1**: 创建 `crates/dasclaw_shell_command/` + Cargo.toml（依赖 `shlex`, `tree-sitter-bash`, `codex_protocol` path-dep 直至 dasclaw_protocol 落地）
2. **C2**: `lib.rs` + `shell_detect.rs` + `command_safety/` verbatim
3. **C3**: `bash.rs` + `powershell.rs` verbatim
4. **C4**: `parse_command.rs` verbatim（最大文件，单独提交便于 review）
5. **C5**: codex `shell-command/tests/` 中 `parse_command_*.rs` fixture 选 prefix-match + powershell-detect 两组 verbatim port
6. **C6**: `scripts/check_codex_shell_command_drift.py` + workflow 接入
7. **C7**: doc 32 §W4 / doc 35 文本同步（ironclaw UI 接入点说明）

**验收命令**：

```bash
cargo nextest run -p dasclaw_shell_command
cargo clippy --no-deps -p dasclaw_shell_command --all-targets -- -D warnings
python3.12 scripts/check_no_panics.py --base origin/xClaw
python3   scripts/check_codex_shell_command_drift.py
```

---

## 5. Rejected alternatives

### 5.1 把 `dasclaw_bash_validation` 重写在 `dasclaw_shell_command` 之上

见 §3.3 ❌ —— 解析 vs 决策不同语义。

### 5.2 不 port，由 ironclaw UI 自己实现一份命令解析

- **拒绝理由**：上游 `parse_command_impl` 2,526 LOC、覆盖大量 bash/PowerShell edge case，自己写一份就是 reinvent。

### 5.3 仅 port `command_safety/`（is_safe_command）作为 dasclaw_bash_validation 增强

- **拒绝理由**：`is_safe_command` 接受**已 parsed** 的 `ParsedCommand`，没有 `parse_command.rs` 单 port 它意义不大。

### 5.4 与 `dasclaw_bash_validation` 合并成一个 crate

- **拒绝理由**：违反 ADR-129 §1.3 verbatim 红线 — codex `shell-command` 是独立 crate，合并就是重构。

---

## 6. References

- codex-rs/shell-command/src/lib.rs：上游公共 API
- codex-rs/shell-command/src/parse_command.rs:23：`DO NOT REVIEW THIS CODE BY HAND` 注释（坚定 verbatim 选择）
- [ADR-129 §1.3](adr-129-sandbox-windows-windows-crate-adoption.md) — verbatim 红线模板
- [ADR-132](adr-132-execpolicy-starlark-port-plan.md) — 同一 port 模式
- [#326](https://github.com/Linnanli/xClaw/issues/326) Part 3a — issue 自带研究范围
- [`crates/dasclaw_bash_validation/src/lib.rs`](../../../crates/dasclaw_bash_validation/src/lib.rs) — claw-code 派生现状

---

## 7. Sign-off

| 角色 | 姓名 | 状态 | 时间 |
|---|---|---|---|
| Architect | nally | ⏳ pending | — |
| Author | GitHub Copilot agent | ✅ drafted | 2026-05-08 |
