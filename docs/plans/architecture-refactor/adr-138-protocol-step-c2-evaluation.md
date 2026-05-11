# ADR-138: dasclaw_protocol Step C2 — Full crate verbatim port evaluation (research-only)

- **Status**: 🟡 **Decision: pending — recommends 2B "keep file-level slice"**（research-only ADR；implementation **out of scope** for this PR）
- **Date**: 2026-05-XX
- **Approver**: pending nally sign-off
- **Authors**: GitHub Copilot agent
- **Tracker**: #324 sub-task 2（dasclaw_protocol Step C2 评估窗口）
- **Related**:
  - [ADR-129](adr-129-sandbox-windows-windows-crate-adoption.md) §1.3 — verbatim 红线
  - [ADR-133](adr-133-shell-command-adoption-eval.md) §2.5 — 文件级 slice 先例
  - [ADR-136](adr-136-protocol-expansion-plan.md) §3 — Step C1 file-level slice + 末尾"Step C2 留给 ADR-138"
  - [ADR-137](adr-137-net-proxy-port-plan.md) — 姊妹 verbatim port ADR
  - codex 上游：`codex-cli-main/codex-rs/protocol/`（28 *.rs + 1 asset，~16K LOC）
  - 现状：[`crates/dasclaw_protocol/`](../../../crates/dasclaw_protocol/)（28 *.rs + 1 asset，file-level verbatim slice，drift-guarded）
  - 已落地 Step C1 PR：[#382](https://github.com/Linnanli/xClaw/pull/382) / [#389](https://github.com/Linnanli/xClaw/pull/389) / [#391](https://github.com/Linnanli/xClaw/pull/391) / [#393](https://github.com/Linnanli/xClaw/pull/393) / [#396](https://github.com/Linnanli/xClaw/pull/396) / [#398](https://github.com/Linnanli/xClaw/pull/398) / [#400](https://github.com/Linnanli/xClaw/pull/400) / [#402](https://github.com/Linnanli/xClaw/pull/402) / [#404](https://github.com/Linnanli/xClaw/pull/404)

---

## 1. Context

### 1.1 Step C1 闭环现状（2026-05-XX）

ADR-136 §3 amendment 2 规划的 10 PR 拆分已实际落地 9 个 PR（C1.4a/C1.4b 在 PR #402 中合并为单 PR Layer-3 hub，详见 PR #402 描述）：

| PR | 内容 | 状态 |
|---|---|---|
| #382 | C1.1 rename `dasclaw_parsed_command` → `dasclaw_protocol` + drift guard | ✅ |
| #389 | C1.2 Layer 1（14 叶子）| ✅ |
| #391 | C1.3 Layer 2（`config_types` ↔ `openai_models` cycle）| ✅ |
| #393 | C1.prep-1 vendor `dasclaw_async_utils` file-level slice | ✅ |
| #396 | C1.prep-2 vendor `dasclaw_utils_string` file-level slice | ✅ |
| #398 | C1.prep-3 vendor `dasclaw_utils_cache` file-level slice | ✅ |
| #400 | C1.prep-4 vendor `dasclaw_utils_image` file-level slice | ✅ |
| #402 | C1.4 Layer 3 hub（7 files：`protocol` / `permissions` / `models` / `request_permissions` / `approvals` / `network_policy` / `items`）| ✅ |
| #404 | C1.5 Layer 4（`error.rs` + `error_tests.rs`，含 Linux-only `landlock` / `seccompiler` target deps）| ✅ |

净增量：

| 维度 | 数值 |
|---|---|
| 新增 verbatim *.rs | 28 |
| 新增 asset（embeds/prompts）| 1 |
| 新建小 vendor utils crate | 4（`dasclaw_async_utils` / `dasclaw_utils_string` / `dasclaw_utils_cache` / `dasclaw_utils_image`）|
| drift script PAIRS 行 | 25 → 27 |
| `cargo nextest run -p dasclaw_protocol` | 188 ✅ |
| 上游覆盖率（按文件数）| 28/28 = **100%** |

### 1.2 Step C1 与 upstream `codex-protocol` 的 实际差异

> `diff codex-cli-main/codex-rs/protocol/{Cargo.toml,src/lib.rs} crates/dasclaw_protocol/{Cargo.toml,src/lib.rs}` 实测。

| 维度 | upstream `codex-protocol` | 现行 `dasclaw_protocol` |
|---|---|---|
| crate name | `codex-protocol` | `dasclaw_protocol` |
| lib name | `codex_protocol` | `dasclaw_protocol`（隐式由 crate name 推出）|
| 版本管理 | `version.workspace = true` + `[workspace.dependencies]` 中央管理 | 单 crate 内联 `chrono = "0.4.43"` / `reqwest = "0.12"` 等版本 pin |
| `[dependencies]` 总行数 | 22 行 + `[lints] workspace=true` | 26 行（多 4 行 Linux-only `[target.'cfg'.dependencies]`，但 dev/build/utils 替换为 `dasclaw_*` path）|
| Linux-only deps | 在上游 workspace 根 `[workspace.dependencies]` 注册，protocol 自身只 cfg-import | **本仓显式声明** `[target.'cfg(target_os="linux")'.dependencies]` `landlock="0.4.4"` + `seccompiler="0.5.0"`（与上游 `codex-cli-main/codex-rs/protocol/Cargo.toml:48-51` 行为同形）|
| 上游 transitive utils dep | `codex-utils-template`（**未被 28 个文件中任何一个 `use`**）| 本仓 `Cargo.toml` 已合理省略 |
| `*.rs` 内容 | — | **byte-for-byte equal**（drift guard 每 PR 验证）|

**关键事实**：`*.rs` 内容已经完全 verbatim。差异只在 `Cargo.toml` 包元数据 + lib name + 版本管理风格。

### 1.3 Step C2 的真正决策点是什么

ADR-136 §3 末尾原文：
> "等 step C1 5 个 PR 全部落地后，再起 ADR-138 评估"是否值得吸收剩余 ~3,500 LOC + 4 transitive utils crate"。"

按 §1.1 已落地结果，"剩余 ~3,500 LOC"已变成 **0 LOC**（28/28 文件 100% 已 port），"4 transitive utils crate"也已全部落地为独立 dasclaw 小 crate。所以"是否值得吸收"这个原命题**已被 Step C1 amendment 2 实质完成**。

剩余 Step C2 候选范围只有：

1. **R-C2-a** — 把 crate name 从 `dasclaw_protocol` 改回 `codex-protocol` 上游名（lib name 同步改 `codex_protocol`）。
2. **R-C2-b** — 把 `[dependencies]` 中所有内联版本 pin 改成 `workspace = true`，引入 x-claw-wide `[workspace.dependencies]` 注册表（牵动整个 workspace，不是 protocol 单 crate 决策）。
3. **R-C2-c** — vendor 上游 `codex-utils-template`（**当前 28 文件中无 `use`，因此非必需**）。
4. **R-C2-d** — 移除本仓 `[target.'cfg(target_os="linux")'.dependencies]` 显式声明，迁到 x-claw workspace 中央（同 R-C2-b，牵动 workspace）。

---

## 2. Decision options

### Option 2A — Full crate verbatim port

- 直接复用上游 `Cargo.toml`（含 `[workspace.dependencies]` + `name = "codex-protocol"` + `[lints] workspace=true`）
- crate name 改 `codex-protocol`、lib name 改 `codex_protocol`、所有 consumer 改 `use codex_protocol::*`
- 必须先建 x-claw 根 `[workspace.dependencies]` 注册表（侵入全仓所有 23+ crate）

**优点**：与上游 `Cargo.toml` byte-equal，drift guard 可纳入 `Cargo.toml` 也比对。

**缺点**：
- 命名层与 ADR-114 "dasclaw rebrand" 红线冲突——所有 consumer crate 都得引入 `extern crate codex_protocol as dasclaw_protocol;` shim 或全仓改名
- 引入 x-claw `[workspace.dependencies]` 是一次性高 cost 决策，应该独立 ADR（不该被 protocol Step C2 顺带绑架）
- 与 ADR-137 PR-N23 `dasclaw_net_proxy` 已实证的"file-level slice + 本地包元数据"范式不一致

### Option 2B — Keep file-level slice + 维持 drift guard（**推荐**）

- 保留 `crate-name = dasclaw_protocol`、保留内联版本 pin、保留本仓 `[target.'cfg'.dependencies]`
- 不 vendor `codex-utils-template`（未被 use，无意义）
- drift guard 继续覆盖 28 个 *.rs + 1 asset 的 byte equality
- 把"上游 *.rs 新增或修改"通过 drift script 失败立即上推到本仓 PR（已实证机制工作良好）

**优点**：
- 维持 §1.2 表格中所有现行选择（已被 9 PR + 188 nextest 验证）
- 与 ADR-137 / ADR-133 §2.5 / ADR-114 同源一致
- workspace.dependencies 重构留给单独决策，不绑架本 ADR

**缺点**：
- 上游 `Cargo.toml` 变更（新增 dependency / 调版本）**不进** drift guard ⇒ 需要人工同步
- 上游若把某文件 `use` 改到 `codex-utils-template`，本仓得即时 vendor `dasclaw_utils_template`（同 prep-1..4 范式）

### Option 2C — 混合：仅 vendor 缺失 utils 小 crate（unmeaning）

按 §1.1，4 个 utils 小 crate 已全部落地，且 `codex-utils-template` 未被 use，因此 Option 2C 蜕变为 **Option 2B 的同义改写**——不再作为独立选项。

---

## 3. Recommendation

**采纳 Option 2B**，理由 = §1.3 + §2 三段对比：

1. ADR-136 §3 末尾给 Step C2 留的真正 open question（"是否值得吸收剩余文件 / utils crate"）已被 Step C1 amendment 2 实质完成。
2. 唯一剩下的差异是 `Cargo.toml` 包元数据风格 + crate name；这两项**不是 verbatim 红线诉求**（ADR-129 §1.3 立法目的是 *.rs 行为同形，已达成）。
3. crate name 翻转牵涉 ADR-114 全仓 rebrand 红线；workspace.dependencies 引入是独立 ADR 议题。两者都不该被 protocol Step C2 绑架。

**形式落地**：

- 本 ADR-138 merge 后，ADR-136 §3 末尾"Step C2 留给 ADR-138 评估"条目改写为 "Step C2 决策 = ADR-138 Option 2B：dasclaw_protocol 保持文件级 slice，drift guard 长期维持"。
- 后续若上游 `codex-utils-template` 被新增 file 引用，按 prep-N 范式增开"Step C2 follow-up PR"vendor `dasclaw_utils_template`，不需要重开 ADR。
- 后续若决定引入 `[workspace.dependencies]`，单独起 ADR-XXX；本 ADR 不预判其结论。

---

## 4. Out of scope（本 ADR PR）

❌ **不写一行 Rust 代码** — 本 PR 仅落 ADR-138 文档 + 同步更新 31/32 的 crate 清单 / W6 任务表。

❌ 不改 `crates/dasclaw_protocol/Cargo.toml`。

❌ 不动 `[workspace.dependencies]`（本仓目前**未启用** workspace.dependencies 表）。

❌ 不 rename crate name / lib name。

❌ 不 vendor `codex-utils-template`（因 28/28 文件均不 use）。

❌ 不重启 Step C1（PR-C1.1 ~ C1.5 已闭环）。

---

## 5. Validation（本 ADR PR）

```bash
python3.12 scripts/check_no_panics.py --base origin/xClaw           # OK (no .rs changed)
python3   scripts/check_no_new_ironclaw_literal.py --base origin/xClaw   # OK
python3.12 scripts/check_codex_protocol_drift.py                    # OK (no vendored *.rs / Cargo.toml moved)
cargo fmt --all -- --check                                          # OK (no .rs changed)
```

---

## 6. Open questions（留给 reviewer）

1. **Q1 — `[workspace.dependencies]` 何时启用？**：本仓 23+ crate 当前均内联版本 pin。若 W7+ 决定引入 workspace.dependencies，建议单独起 ADR，不在 ADR-138 范围。
2. **Q2 — `codex-utils-template` 监控**：drift script 现行只比对 28 个 *.rs + 1 asset。若上游某 `*.rs` 新增 `use codex_utils_template::*`，drift script 会失败但不会自动指出根因——是否在 drift script 加 "use 行白名单"校验？建议在本 ADR merge 后单独提小 PR 加一行校验。
3. **Q3 — Linux-only target dep 维护**：本仓 `[target.'cfg(target_os="linux")'.dependencies]` 中 `landlock="0.4.4"` + `seccompiler="0.5.0"` 当前与上游 pin 一致。是否纳入 drift script？建议同 Q2 一并加 use 行 + Cargo.toml 关键字段（target deps section）的白名单校验。

---

## 7. Decision log

- **2026-05-XX**：起草 ADR-138。背景为 Step C1 amendment 2 落地 9 PR + 4 小 utils crate 后，原 Step C2 评估命题"剩余 3.5K LOC + 4 transitive utils crate"已被 amendment 2 实质完成。本 ADR 推荐 Option 2B "keep file-level slice"，把剩余的 crate-name / workspace.dependencies 议题驱逐到独立 ADR 范围。等待 nally sign-off。
