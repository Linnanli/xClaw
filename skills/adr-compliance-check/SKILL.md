---
name: adr-compliance-check
description: "Verify current git changes comply with x-claw ADR redlines: verbatim port purity, drift-guard wiring, CI roll-up failure-check, PR description ADR/Issue citations, starlark pin, .ironclaw literal ban."
---

# ADR Compliance Check

## 目标

x-claw 仓库依赖一组 ADR（Architecture Decision Records，位于 `docs/plans/architecture-refactor/`）维持架构纪律。`code-review-expert` 关注 SOLID/安全/性能；`code-quality-audit` 关注代码工艺；本 skill 专门检测**架构层面 ADR 红线**是否被违反——这类问题在传统 review 工具里容易漏。

## 严重等级

| Level | 含义 | 处置 |
|-------|------|------|
| **R0** | ADR 红线违规（verbatim 港口被手改 / 红线 label PR 被 agent 操作 / `.ironclaw` 字面量复活 / starlark 版本被改动） | **必须**阻断合并 |
| **R1** | ADR 接线不完整（drift guard 缺失 / CI roll-up 没接新 guard / failure-check stanza 缺失 / PR 没 cite ADR 或 issue） | 合并前必须补 |
| **R2** | ADR 软约束未遵守（PR 描述 4 块缺一 / stacked PR 没说明 base 与 merge 顺序 / 没贴 skill 自查产出） | 同 PR 内补 |
| **R3** | 风格性提醒（commit message 前缀对不上 / 文档命名不一致） | 可选改进 |

## Workflow

### 1) 收集变更上下文

- `git status -sb`、`git diff --stat origin/xClaw...HEAD`、`git diff origin/xClaw...HEAD`
- 用 `gh pr view` 拉取 PR 描述（如已开 PR）
- 识别本次改动**触及哪些 crate / 哪些 ADR 区域**：
  - `crates/dasclaw_<X>/` 是否对应 codex 上游 verbatim port
  - `crates/dasclaw_hooks/` → ADR-113 hook 引擎统一
  - `desktop-client/` 含 `.ironclaw` / `IRONCLAW_BASE_DIR` 字面量 → ADR-114
  - 修改 `.github/workflows/code_style.yml` → CI roll-up 接线
  - 修改 `Cargo.toml` 的 starlark 版本 → ADR-132

### 2) Verbatim port 纯度检查（ADR-129 §1.3）

任何 `crates/dasclaw_<X>/` 目录的变更，先确认是否声明为 verbatim port：

- `Cargo.toml` 顶部注释是否写明 "Verbatim port of codex-X" + 上游路径
- 是否存在对应的 drift guard：`scripts/check_codex_<X>_drift.py`

若是 verbatim port，**只允许两种机械编辑**：

1. `Cargo.toml` 包名 / 依赖名 swap（`codex-X` → `dasclaw_X`）
2. `src/*.rs` 与 `tests/*.rs` 的 use-path swap（`use codex_X` → `use dasclaw_X`）

任何其他改动（重命名函数、加 cfg、改字符串字面量、加 doc-comment、加 panic、加 import）一律 **R0 红线违规**。

规范的 drift-guard 脚本结构（`PAIRS` / `SWAP_PATTERNS` / `normalize`）参考 `scripts/check_codex_execpolicy_drift.py`（带 use-path 归一化）与 `scripts/check_codex_process_hardening_drift.py`（纯字节比对）。

**典型反模式**：
- 把 `CODEX_HOME` 改成 `DASCLAW_HOME`（env var 字面量必须 verbatim 保留）
- 把 `.codex` 后缀改成 `.dasclaw`（同上）
- 在 verbatim 文件里加 `// dasclaw-specific:` 注释
- 把 `unwrap()` 改成 `expect("...")`
- 在尾部追加 `if cfg!(dasclaw)` 分支（**补丁式代码**双红线，同时违反 ADR-129 §1.3 + AGENTS.md §"不要碰的红线"）

### 3) Drift guard 完整性检查

每个 verbatim port 必须满足：

- `scripts/check_codex_<X>_drift.py` 存在且 `chmod +x`
- 脚本里 `PAIRS` 列表覆盖 **本地 src 树下所有文件**（含 tests/、examples/）
- 若有 use-path swap，`SWAP_PATTERNS` 必须把所有 `dasclaw_*` 反向归一为 `codex_*`
- `.github/workflows/code_style.yml` 必须有同名 job
- `code-style` roll-up job 的 `needs:` 数组必须包含该 job
- roll-up `run:` 块必须有 failure-check stanza（`if [[ "${{ needs.codex-<X>-drift.result }}" != "success" ]]; then exit 1; fi`）

四样缺一即 R1。

### 4) `.ironclaw` 字面量检查（ADR-114）

- `git diff origin/xClaw...HEAD` 里搜 `\.ironclaw|IRONCLAW_BASE_DIR`
- 任何**新增**位置（除已知豁免列表 `scripts/check_no_new_ironclaw_literal.py` 维护）一律 R0
- `python3 scripts/check_no_new_ironclaw_literal.py --base origin/xClaw` 必须 OK
- CI 该 job `skipped` 仅在 PR 显式打了 `adr-114-class-b` label 时被允许（参考 `.github/workflows/code_style.yml` 的 `no-new-ironclaw-literal.if:` 条件）；其他场景 `skipped` 也算 R1

### 5) Starlark 版本 + 其他精确钉死依赖（ADR-132）

- `grep -r 'starlark' Cargo.toml crates/*/Cargo.toml` → 必须 `="=0.13.0"`（**带等号 + 双引号**），不允许 `^0.13` / `>=0.13`
- 修改任何精确钉死的依赖版本时，PR 描述必须 cite 控制 ADR 并解释回归测试

### 6) Hook 引擎统一（ADR-113）

- 编译期断言 `const _: () = assert!(dasclaw_hooks::count_hook_systems() == 1);` 必须保留
- 不允许新增第二套 hook 派发体系（参考 `crates/dasclaw_hooks/src/lib.rs`）
- 改动 `dasclaw_hooks` 公共 API 必须 cite ADR-113

### 7) Panic / unwrap 红线（AGENTS.md §"不要碰的红线"）

- `python3.12 scripts/check_no_panics.py --base origin/xClaw` 必须 OK
- 仅测试代码、`mod tests`、`#[cfg(test)]` 块内允许 `unwrap()/expect()`
- 生产代码新增 `panic!()` 一律 R0

### 8) PR 描述 / 体例（AGENTS.md §"PR 必填项"）

- 标题前缀对应类别：`feat(<area>):` / `fix(<area>):` / `docs(<area>):` / `chore(<area>):` / `test(<area>):`
- Body 必须包含：
  - **背景 / 目标**
  - **改动范围**
  - **非目标（What's NOT in this PR）**
  - **验证（命令 + 结果）**
- Body 必须含 `Closes #<issue>` 或 `Refs #<issue>` —— 缺它 board 状态机不会推进
- `Closes` 仅当本 PR 是 issue 的最后一个交付动作；多 PR 拆分时早期 PR 用 `Refs`，最后一个用 `Closes`
- stacked PR 必须额外有：`base 分支不是 xClaw`、`merge 顺序`、`先看 #X 再看本 PR`

四块缺一为 R2，缺 issue 引用为 R1。

### 9) 红线 label 检查（AGENTS.md §"不要碰的红线"）

- 若 PR 关联的 issue 含 `adr-redline` label，agent **禁止**任何自动操作 → R0
- 注：`--no-verify` / `git push --force` 类骚操作属于 commit/push 行为，diff 检测不到；本 skill 不强制检查，由 CI / branch protection 兜底

### 10) Skills 自查产出黏贴检查（AGENTS.md §"完成后"）

- 触及 ADR 红线的 PR（任何 `crates/dasclaw_*` / drift guard / CI roll-up / starlark pin / `.ironclaw` 改动）**必须**贴 `adr-compliance-check` 输出，不能用其他 skill 替代
- 其他非平凡 PR 至少贴 `code-quality-audit` + `code-review-expert` 任一
- 缺失为 R2

### 11) 输出格式

```markdown
## ADR Compliance Report

**Files reviewed**: X files, Y lines changed
**ADR scope**: ADR-XXX (verbatim port) + ADR-YYY (...)
**PR**: #NNN (or "no PR yet")

### Findings

#### R0 — Redline violations
- [ ] (issue + suggested fix)

#### R1 — Wiring incomplete
- [ ] (issue + suggested fix)

#### R2 — Soft conventions
- [ ] (issue + suggested fix)

#### R3 — Style nits
- [ ] (issue + suggested fix)

### Summary

- ADR-129 §1.3 verbatim purity: PASS / FAIL (reason)
- Drift-guard wiring: PASS / FAIL
- ADR-114 `.ironclaw` literal ban: PASS / FAIL
- ADR-132 starlark pin: PASS / FAIL / N/A
- ADR-113 hook engine unity: PASS / FAIL / N/A
- Panic/unwrap policy: PASS / FAIL
- PR description completeness: PASS / FAIL / N/A (no PR yet)
- Skill self-audit output attached: PASS / FAIL / N/A

### Recommendation

`block-merge` | `fix-before-merge` | `fix-in-followup` | `approve`
```

## 何时使用

| 场景 | 是否触发 |
|------|---------|
| 任何触及 `crates/dasclaw_*` 的 PR | **是** |
| 触及 `.github/workflows/code_style.yml` 的 PR | **是** |
| 触及 `scripts/check_codex_*_drift.py` 的 PR | **是** |
| 修改 `Cargo.toml` 中 starlark / 其他 ADR 钉死依赖 | **是** |
| 触及 `crates/dasclaw_hooks/` 公共 API | **是** |
| desktop-client/ 改动 | 是（重点查 `.ironclaw` 字面量） |
| 纯文档 / 单字符 typo | 否 |
| 仅 `cargo fmt` 自动修复 | 否 |

## 与其他 skill 的关系

```
新代码完成
  ↓
code-quality-audit          # 代码工艺：长函数/重复/unwrap 滥用
  ↓
code-simplifier             # 收敛：消嵌套/去重/改命名（verbatim port 跳过！）
  ↓
adr-compliance-check        # 架构纪律：本 skill
  ↓
code-review-expert          # 综合 review：SOLID/安全/性能
  ↓
push + 开 PR
```

verbatim port 场景下 `code-simplifier` 必须跳过（任何"简化"都违反 ADR-129 §1.3）；本 skill 在那种场景下尤其关键。

## 参考

- `AGENTS.md` §"不要碰的红线" / §"Skills 强制使用规范" / §"PR 必填项"
- [`docs/plans/architecture-refactor/adr-113-hook-engine-unification.md`](../../docs/plans/architecture-refactor/adr-113-hook-engine-unification.md) — hook 引擎单一化
- [`docs/plans/architecture-refactor/adr-114-dasclaw-rebrand.md`](../../docs/plans/architecture-refactor/adr-114-dasclaw-rebrand.md) — `.ironclaw` / `IRONCLAW_BASE_DIR` 字面量禁令 + Class-B 豁免
- [`docs/plans/architecture-refactor/adr-129-sandbox-windows-windows-crate-adoption.md`](../../docs/plans/architecture-refactor/adr-129-sandbox-windows-windows-crate-adoption.md) §1.3 — verbatim port 红线（核心权威）
- [`docs/plans/architecture-refactor/adr-132-execpolicy-starlark-port-plan.md`](../../docs/plans/architecture-refactor/adr-132-execpolicy-starlark-port-plan.md) — starlark `=0.13.0` 钉死 + drift guard 模板
