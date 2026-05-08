# ADR-132: `dasclaw_execpolicy` Starlark engine — port plan

- **Status**: � **Accepted (executed)** — port landed in [#326 Part 2](https://github.com/Linnanli/xClaw/issues/326). Originally accepted plan-only on PR #340 (`4da6b0ae`); the verbatim port (1,790 LOC + 617 LOC `dasclaw_absolute_path` dep) and `scripts/check_codex_execpolicy_drift.py` landed in the follow-up PR. See §2.4 / §3.3 for amendments noted during execution.
- **Date**: 2026-05-08
- **Approver**: pending nally sign-off
- **Authors**: GitHub Copilot agent
- **Tracker**: [#326](https://github.com/Linnanli/xClaw/issues/326) Part 2 — `dasclaw_execpolicy` Starlark engine 完整 port (B-1)
- **Related**:
  - [ADR-129](adr-129-sandbox-windows-windows-crate-adoption.md) §1.3 — verbatim port red line（同一执行模型套用到本 port）
  - [doc 32 §W5](32-execution-plan.md) — "MCP 统一 + ExecPolicy" wave
  - [doc 35 §I](35-codex-capability-inventory.md) — ExecPolicy 能力差距条目
  - codex 上游：`codex-cli-main/codex-rs/execpolicy/`（10 files, 1,790 LOC）
  - dasclaw 现状：[`crates/dasclaw_execpolicy/src/lib.rs`](../../../crates/dasclaw_execpolicy/src/lib.rs)（30-LOC stub）

---

## 1. Context

### 1.1 上游事实（三层验证）

`codex-cli-main/codex-rs/execpolicy/` 是一个独立 crate（已三层验证：`semantic_search "execpolicy starlark"` → `vscode_listCodeUsages` on `Policy::evaluate` → `rg "use codex_execpolicy"`），共 10 个源文件、1,790 LOC：

| 文件 | LOC | 角色 |
|---|---|---|
| `lib.rs` | 27 | 公共 API 出口 |
| `main.rs` | 18 | CLI（policy lint） |
| `parser.rs` | 472 | Starlark 规则解析（builtins: `define_program`, `path`, `flag`, `arg`...）|
| `policy.rs` | 375 | `Policy` struct + `evaluate(argv, cwd, env) -> Decision` |
| `rule.rs` | 306 | `Rule` 枚举（`PrefixRule`, `ArgRule`, `NetworkRule`, ...）|
| `decision.rs` | ~120 | `Decision`（`Match`, `NoMatch`, `Forbidden`）|
| `amend.rs` | ~140 | 规则修改 / 合并 |
| `error.rs` | ~80 | 错误类型 |
| `executable_name.rs` | ~110 | 可执行文件名归一化 |
| `execpolicycheck.rs` | ~140 | 集成式检查入口 |

依赖（来自 `codex-cli-main/codex-rs/execpolicy/Cargo.toml`）：
- **`starlark = "0.13"`**（facebook/starlark-rust 主线）
- `allocative` `~0.3`（Starlark heap 报告）
- `serde` / `serde_json` / `thiserror` / `anyhow` / `tracing` / `regex-lite`

### 1.2 dasclaw 现状（已三层验证）

[`crates/dasclaw_execpolicy/src/lib.rs`](../../../crates/dasclaw_execpolicy/src/lib.rs) 共 30 行：

```rust
pub enum PolicyDecision { Allow, Deny }
// + 1 skeleton test
```

无 Starlark 解析器、无规则引擎、无 codex 规则集解析能力。doc 35 §I（[35-codex-capability-inventory.md#L48](35-codex-capability-inventory.md#L48)）已经把"完整移植到 dasclaw_execpolicy"列入 ⭐⭐⭐⭐⭐ 价值条目。

### 1.3 为什么需要先有 ADR

#326 Part 2 issue body 明确：

> **先决**：新 ADR `adr-1XX-execpolicy-port.md` 锁 starlark-rust 版本 + 公共 API 范围

不是为了仪式感，而是因为：

1. **Starlark DSL 是对外契约**。一旦 `dasclaw_execpolicy` 接受 codex 规则集，规则文件版本管理 / breaking change 都要走 ADR；
2. **starlark-rust 大版本变化频繁**（0.10 → 0.13 之间 builtins API 大改），不锁版本会让升级变成 patch 战；
3. **公共 API 范围**决定了 `dasclaw_governance` / `dasclaw_sandbox` 接入点是否稳定，必须先冻结再接。

---

## 2. Decision

### 2.1 执行模型：verbatim port（套用 ADR-129 §1.3 红线）

参考 ADR-129/130 在 `dasclaw_sandbox_windows` 上的成功经验，`dasclaw_execpolicy` 同样采用**上游字面对齐 + 仅做命名改写**的 port 模式：

| 允许的机械改写 | 禁止 |
|---|---|
| Cargo.toml `name = "codex-execpolicy"` → `name = "dasclaw_execpolicy"` | Rust 化重构、把多个文件合并、提取 helper |
| `use codex_execpolicy::X` → `use dasclaw_execpolicy::X` | 改 trait 边界、加新 enum variant |
| 顶层 doc-comment 注明 "Derived from openai/codex commit \<sha\>" | 任何"补丁式"修改（在尾部加分支、复制粘贴逻辑） |
| 接入层 helper（在新 module 里）添加 dasclaw 特有的 `tracing` | 修改 starlark builtins 行为 |

**端口提交记录** sha 的来源：`codex-cli-main/codex-rs/execpolicy/` 当前所跟踪的 codex commit（与 `dasclaw_sandbox_windows` verbatim 起点同源 `6e838a19fa`，若提交时点已更新需在 PR 描述中注明）。

### 2.2 starlark-rust 版本锁

```toml
# crates/dasclaw_execpolicy/Cargo.toml
[dependencies]
starlark    = "=0.13"          # exact-pin, see ADR-132 §2.2
allocative  = "~0.3"
serde       = { version = "1", features = ["derive"] }
serde_json  = "1"
thiserror   = "1"
anyhow      = "1"
tracing     = "0.1"
regex-lite  = "0.1"
```

**`=0.13` exact-pin** 而非 `^0.13`：starlark-rust 的 builtins 注册 API（`#[starlark_module]`）在 0.10→0.11→0.12→0.13 之间多次破坏性调整，本 ADR 只锁 0.13；任何后续升级（含 0.14）必须走新 ADR + 配套 fixture re-run。

### 2.3 公共 API 范围（冻结）

`dasclaw_execpolicy::lib.rs` 出口（首批）：

```rust
// 类型出口（来自 verbatim port 后的内部模块）
pub use policy::{Policy, PolicyBuilder};
pub use decision::{Decision, MatchedRule};
pub use rule::{Rule, RuleKind};
pub use error::{ExecPolicyError, ParseError};
pub use parser::parse_policy;

// 入口函数
pub fn load_policy_from_dir(path: &Path) -> Result<Policy, ExecPolicyError>;
pub fn load_policy_from_str(src: &str, name: &str) -> Result<Policy, ParseError>;

// 评估入口
impl Policy {
    pub fn evaluate(&self, argv: &[String], env: &HashMap<String, String>) -> Decision;
}
```

**冻结边界**：以上签名等同 codex 上游公共 API 的 1:1 镜像（仅 crate 名替换）。任何新增字段、新增 enum variant、删除任何条目，都必须走配套 ADR / breaking-change PR。

`dasclaw_governance` / `dasclaw_sandbox` 在执行前查询 `Policy::evaluate(argv, env)` 即可拿 `Decision`；**不再**通过现有 30-LOC stub 的 `PolicyDecision` 枚举（stub 在 port 落地 PR 中删除）。

### 2.4 fixture 选取（验收用）

issue body 验收标准：

> codex `execpolicy/tests/*` 中至少 prefix_rule + network_rule 两组用例 port 后通过

**执行期修正（2026-05）**：上游 `codex-cli-main/codex-rs/execpolicy/tests/` 实际仅含**单一文件** `basic.rs`（963 LOC，33 个 `#[test]`），覆盖 prefix / network / alias / host-executable / justification / strictest-decision 等所有用例。verbatim 红线要求保留上游文件结构，因此本 ADR 原表（拆 `tests/prefix_rule.rs` + `tests/network_rule.rs` 两文件）作废，改为：

| 上游文件 | 本仓文件 | 覆盖 |
|---|---|---|
| `tests/basic.rs` | `crates/dasclaw_execpolicy/tests/basic.rs`（verbatim） | 33 个 `#[test]`，含 prefix_rule / network_rule / alias / host-executable / strictest-decision 全部门类 |

verbatim port 后 **必须不修改 assertion**；任何与上游不一致都视为 port 错误。验收命令 `cargo nextest run -p dasclaw_execpolicy` 必须 33/33 通过。

**额外发现（2026-05）**：`codex-execpolicy` 对 `codex-utils-absolute-path` 有硬依赖（`AbsolutePathBuf` 用作 `host_executables_by_name` 的 value 类型与 `Rule::with_resolved_program` 入参）。verbatim 红线不允许把它替换成 `std::path::PathBuf`，所以 port 同时**新建** `crates/dasclaw_absolute_path/` 作为 `codex-utils-absolute-path`（617 LOC，dirs/dunce/schemars/ts-rs/serde 五项依赖）的 verbatim sibling crate。该 sibling 同样列入 `scripts/check_codex_execpolicy_drift.py` 的哈希检查范围。

---

## 3. Consequences

### 3.1 Positive

- **零行为差异承诺**：与 `dasclaw_sandbox_windows` 同一红线（ADR-129 §1.3），CI grep guard `scripts/check_no_panics.py` + 后续将加的 `scripts/check_codex_execpolicy_drift.py`（ADR-132 §3.3）保证 verbatim。
- **dasclaw_governance 接入零变更预算**：API 冻结后，governance / sandbox 端可以直接对 `Policy::evaluate` 编程，免反复 follow upstream。
- **starlark-rust 升级路径清晰**：必须配 ADR + fixture re-run，避免 `dasclaw_sandbox_windows` 早期 base-broken（windows 0.58 PCWSTR vs PWSTR）那种被 transitive 升级"暗中改写"的情况。

### 3.2 Negative

- **Starlark 0.13 exact-pin** 牺牲了部分可升级性 — 与其他 dasclaw crate 若后续也用 starlark，需要对齐到 0.13。但目前全 workspace `rg "starlark"` 仅有 stub 引用，无冲突。
- **1,790 LOC + fixture port** 工作量较大，无法塞入一个 PR；下文 §4 给出落地序列。

### 3.3 Mechanical guard（CI 强制）

落地 PR 同时引入：

```python
# scripts/check_codex_execpolicy_drift.py
# 对 crates/dasclaw_execpolicy/src/{amend,decision,error,executable_name,
#   execpolicycheck,lib,main,parser,policy,rule}.rs +
#   tests/basic.rs + examples/example.codexpolicy 与上游同名文件 hash diff,
# 同时覆盖 crates/dasclaw_absolute_path/src/{lib,absolutize}.rs 与上游
# codex-cli-main/codex-rs/utils/absolute-path/src/* 的 hash diff,
# 仅允许 §2.1 表格列出的机械改写（包名 swap + use-path swap）.
# 任何意外 diff 即 fail.
```

与 `crates/dasclaw_sandbox_windows` 的 ADR-129 §1.3 guard 同款。

---

## 4. Implementation sequence（落地 PR 分解）

> **不是**把 Part 2 拆成多 PR — issue body 把 Part 2 整体定为 effort:L、单 PR 验证。本节只是给后续单一 PR 内部的提交序。

1. **C1**: `Cargo.toml` 添加 starlark/allocative/regex-lite 依赖（lockfile 同步）
2. **C2**: `parser.rs` + `executable_name.rs` verbatim port（无外部依赖的最底层）
3. **C3**: `rule.rs` + `decision.rs` + `amend.rs` + `error.rs` verbatim port
4. **C4**: `policy.rs` verbatim port + 删除 30-LOC stub `PolicyDecision`
5. **C5**: `tests/basic.rs` verbatim port（上游单一测试文件 963 LOC / 33 个 `#[test]` — 覆盖 prefix / network / alias / host-executable 全部门类，原 §2.4 表已修正）
6. **C6**: `scripts/check_codex_execpolicy_drift.py` + CI workflow 接入
7. **C7**: doc 32 §W5 / doc 35 §I 落地后状态更新

**验收命令**（issue body 已写明）：

```bash
cargo nextest run -p dasclaw_execpolicy
cargo clippy --no-deps -p dasclaw_execpolicy --all-targets -- -D warnings
python3.12 scripts/check_no_panics.py --base origin/xClaw
python3   scripts/check_codex_execpolicy_drift.py
```

---

## 5. Rejected alternatives

### 5.1 重新发明：Rust-native 规则 DSL

写一个 dasclaw 自己的 DSL（YAML / JSON / serde-derived）。

- **拒绝理由**：
  - 复制 codex 规则集要"翻译"，每次上游更新 = 重新翻译
  - Starlark 已经是事实标准（codex / Bazel / Buck2），社区 / IDE 支持好
  - doc 35 §I 明确"⭐⭐⭐⭐⭐ Starlark 完胜"

### 5.2 不锁版本：`starlark = "0.13"`（caret）

- **拒绝理由**：starlark-rust 不遵守 SemVer 严格语义，0.13 → 0.14 也可能 break；exact-pin 是上游推荐做法。

### 5.3 推迟到 Wave i-7（与 conpty 端口同 PR）

- **拒绝理由**：execpolicy 与 sandbox-windows 完全独立，绑在一起会拖慢两边。本 ADR 与 conpty 端口（待办）解耦。

---

## 6. References

- codex-rs/execpolicy/src/lib.rs：上游公共 API 真理来源
- [ADR-129 §1.3](adr-129-sandbox-windows-windows-crate-adoption.md) — verbatim 红线模板
- [doc 32 §W5](32-execution-plan.md) — 端口落地 wave
- [doc 35 §I](35-codex-capability-inventory.md) — 能力差距登记
- [#326](https://github.com/Linnanli/xClaw/issues/326) Part 2 — issue 自带验收
- [starlark-rust 0.13 release notes](https://github.com/facebook/starlark-rust/releases/tag/starlark-0.13.0) — builtins API 确定文档

---

## 7. Sign-off

| 角色 | 姓名 | 状态 | 时间 |
|---|---|---|---|
| Architect | nally | ⏳ pending | — |
| Author | GitHub Copilot agent | ✅ drafted | 2026-05-08 |
