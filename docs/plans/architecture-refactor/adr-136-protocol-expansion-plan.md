# ADR-136: dasclaw_protocol expansion plan (research-only)

- **Status**: ✅ **Accepted** (path: option 2C two-tier — file-level verbatim slice now, full crate port deferred to ADR-138; implementation **out of scope** for this PR — driven by per-PR sub-ADRs in Step C1.1 ~ C1.5)
- **Date**: 2026-05-08 (drafted) / 2026-05-15 (accepted)
- **Approver**: nally (sign-off in epic [#380](https://github.com/Linnanli/xClaw/issues/380))
- **Authors**: GitHub Copilot agent
- **Tracker**: ADR-135 §6 Q1 — "dasclaw_protocol 切片粒度"
- **Related**:
  - [ADR-129](adr-129-sandbox-windows-windows-crate-adoption.md) §1.3 — verbatim port red line
  - [ADR-132](adr-132-execpolicy-starlark-port-plan.md) — execpolicy port (uses dasclaw_absolute_path)
  - [ADR-133](adr-133-shell-command-adoption-eval.md) — shell-command port (uses dasclaw_parsed_command 31 LOC slice)
  - [ADR-135](adr-135-sandboxing-crate-adoption-eval.md) — sandboxing port (Wave-A 阻塞前置)
  - codex 上游：`codex-cli-main/codex-rs/protocol/`（28 文件，16,053 LOC）
  - dasclaw 现状：[`crates/dasclaw_protocol/`](../../../crates/dasclaw_protocol/)（31 LOC slice，由 PR #342 落地为 `dasclaw_parsed_command`，PR-C1.1 rename 为 `dasclaw_protocol`）
  - 已落地姊妹 verbatim：PR #341 `dasclaw_execpolicy` / PR #342 `dasclaw_shell_command` / PR #343 `dasclaw_process_hardening`

---

## 1. Context

### 1.1 codex-protocol 是什么

`codex-cli-main/codex-rs/protocol/` 是 codex IPC + 配置 + 权限 + 执行结果 + LLM 模型描述的**全协议中枢**。28 个 `.rs` 文件，16,053 LOC（含 624 LOC 内联 `#[cfg(test)]`）：

| 文件 | LOC | 角色 |
|---|---|---|
| `protocol.rs` | 5,238 | **核心** — IPC `Op`/`Event` 大 enum，包含 `SandboxPolicy`、`NetworkAccess`、所有 turn 状态机消息 |
| `models.rs` | 2,774 | LLM 模型描述（`AdditionalPermissionProfile`、`FileSystemPermissions`、`NetworkPermissions` 等）|
| `permissions.rs` | 2,548 | 权限模型（`FileSystemSandboxPolicy`、`NetworkSandboxPolicy`、`ReadDenyMatcher` 等）|
| `openai_models.rs` | 832 | OpenAI 模型清单 |
| `config_types.rs` | 702 | `WindowsSandboxLevel`、模型 config 类型 |
| `error.rs` | 635 | `CodexErr` 全集 |
| `error_tests.rs` | 547 | error 单测 |
| `items.rs` | 449 | 历史项 |
| `mcp.rs` | 360 | MCP 协议类型 |
| `auth.rs` / `account.rs` / `agent_path.rs` / `approvals.rs` | 中等 | 认证 / 账号 / agent path / approval 流 |
| `dynamic_tools.rs` / `exec_output.rs` / `exec_output_tests.rs` | 中等 | 动态 tool / exec 输出 |
| `parse_command.rs` | 31 | ✅ **已切到 dasclaw_parsed_command** (PR #342) |
| 其余 11 文件 | 11-169 | 各种小类型 + 内联 tests |

### 1.2 transitive 依赖图

```
codex-protocol
├── codex-async-utils         ← 未 port
├── codex-execpolicy          ✅ 已落地为 dasclaw_execpolicy (PR #341)
├── codex-network-proxy       ← 未 port (#324 sub-task 3)
├── codex-utils-absolute-path ✅ 已落地为 dasclaw_absolute_path (PR #341)
├── codex-utils-image         ← 未 port
├── codex-utils-string        ← 未 port
└── codex-utils-template      ← 未 port
```

整 crate verbatim port 意味着同时引入 4 个新 vendored crates（async-utils + image + string + template + 待验证的 template 是否在 protocol src 实际被 use）。

### 1.3 ADR-135 提出的需求

[ADR-135](adr-135-sandboxing-crate-adoption-eval.md) §1.3 列出 `dasclaw_sandboxing` 所需的 16 个类型，全部位于 5 个文件：

| 文件 | 需求类型 |
|---|---|
| `permissions.rs` | `FileSystemSandboxPolicy`、`NetworkSandboxPolicy`、`FileSystemPath`、`FileSystemAccessMode`、`FileSystemSandboxEntry`、`FileSystemSandboxKind`、`FileSystemSpecialPath`、`ReadDenyMatcher` |
| `protocol.rs` | `SandboxPolicy`、`NetworkAccess` |
| `models.rs` | `AdditionalPermissionProfile`、`FileSystemPermissions`、`NetworkPermissions` |
| `config_types.rs` | `WindowsSandboxLevel` |
| `error.rs` | `CodexErr` |

但 `permissions.rs` 单文件 2,548 LOC，含彼此交叉引用的 ~50 个类型 + 大量私有辅助；**无法子集化**——切到一个类型必然牵出整个文件。

---

## 2. Decision options

### 2A. **Full verbatim port whole crate**（最 ADR-129-friendly）

把 `codex-cli-main/codex-rs/protocol/` 完整 vendor 进 `crates/dasclaw_protocol/`，16,053 LOC + 4 transitive crates verbatim port：

| 项 | 处理 |
|---|---|
| `crates/dasclaw_protocol/` | 28 文件 verbatim + use-path swap |
| `crates/dasclaw_async_utils/` | **新 crate** verbatim port |
| `crates/dasclaw_utils_image/` | **新 crate** verbatim port |
| `crates/dasclaw_utils_string/` | **新 crate** verbatim port |
| `crates/dasclaw_utils_template/` | **新 crate** verbatim port（如确实被 protocol use） |
| **退役** `crates/dasclaw_parsed_command/` | 31 LOC slice 被 `dasclaw_protocol::parse_command` 取代 |

**优点**：
- ADR-129 §1.3 verbatim 红线 100% 满足
- 未来 codex 升级 `protocol.rs` enum 变体（每周都在加）→ 机械同步
- ironclaw_safety 边界用的就是 codex protocol 那套类型，直接对齐 codex API 表示后续 LLM-driven 升级（model lists / capability 等）零改动
- 所有内联 tests verbatim 落地（624 LOC）

**缺点**：
- **巨大依赖蛋糕** — 4 个新 crate vendored；每个 crate 都需 ADR + drift guard + Cargo workspace member + CI job
- **慢** — 估计 5-7 PR 才能完成（async-utils / image / string / template 各一 PR + protocol 主 PR + 退役 parsed_command 的整理 PR）
- **#324 sub-task 2 / 3 的关键路径变长** — sandboxing port 阻塞在这条线上
- 部分依赖 (`codex_utils_image`) **可能本身不被 sandboxing 需要** — 但 protocol 整 crate 编译需要

### 2B. **Sliced vendor — extend dasclaw_parsed_command into dasclaw_protocol with selected files only**（最快路径）

只 vendor sandboxing 需要的 5 个文件（`permissions.rs` + `protocol.rs` + `models.rs` + `config_types.rs` + `error.rs`）+ 它们依赖的 `lib.rs` 出口 + 已有的 `parse_command.rs`，rename 现有 `dasclaw_parsed_command` → `dasclaw_protocol`。

**优点**：
- 解锁 sub-task 2 走得最快
- 不引入 4 个新 vendored crate

**缺点（致命）**：
- ❌ **违反 ADR-129 §1.3 verbatim 红线**——切片化等于人工挑选公共 API 表面，drift guard 要做"局部 SHA + 局部 sort_use_blocks"，复杂度↑
- ❌ `protocol.rs` 5,238 LOC 中**很多 enum 变体引用 codex-async-utils / codex-utils-image** 的类型；只 vendor 5 个文件做不到，要么删变体（违反 verbatim），要么 stub 缺失类型（违反 verbatim）
- ❌ 上游 `permissions.rs` 与 `protocol.rs` 互相 use；切其中一个等于切两个；切两个等于 7,786 LOC 加上交叉依赖
- ❌ 拒绝

### 2C. **Two-tier — minimal protocol (now) + full protocol (later)**（推荐）

分两步走：
1. **Step C1（眼前）**：把现行 `dasclaw_parsed_command` rename 为 `dasclaw_protocol`，**只追加** `permissions.rs` + `config_types.rs::WindowsSandboxLevel` + `error.rs::CodexErr` + `models.rs` 中 sandboxing 需要的子集（如必须切，也是 verbatim 文件级，不切类型级），覆盖 ADR-135 §1.3 16 类型清单。但**不动 `protocol.rs` 5,238 LOC**——`SandboxPolicy` / `NetworkAccess` 单独抽到 `dasclaw_protocol/src/sandbox_policy.rs` 作为 verbatim slice（如 `protocol.rs` 中定义复杂到无法切，则降级成"protocol.rs 全文件 verbatim port"）
2. **Step C2（之后）**：在 W7+ 把 `dasclaw_protocol` 升级到 codex-protocol 全文件 verbatim port，吸收 `codex_utils_*` 4 crate（同时清理 step C1 留的本地 slice）

**优点**：
- Step C1 解锁 sub-task 2 （sandboxing port），同时**不立即招来 4 个 transitive crate**
- Step C1 的"文件级 verbatim slice"仍受 drift guard 保护（每个文件单独 SHA），ADR-129 §1.3 解读上属于"verbatim 但 partial-files-vendored"
- 前置 ADR-129 §1.3 实施先例：现行 `dasclaw_parsed_command` 已是文件级 slice (`parse_command.rs` 单文件)；step C1 把它扩展到多文件而已

**缺点**：
- 切多个文件比切单个文件**复杂度上升**——但仍然是字节级 SHA 比对，不切类型
- Step C2 需要单独 ADR + PR
- ADR-129 §1.3 红线**需要明确解读**："文件级 verbatim slice" 是否符合精神？倾向认为符合，因为已有 `parse_command.rs` 先例（[ADR-133 §2.5](adr-133-shell-command-adoption-eval.md)）

### 2D. **Defer — 推迟整个 #324 sub-task 2**

不做任何 protocol 扩展，把 #324 sub-task 2 milestone 推到 W7 或之后。

**优点**：诚实声明范围
**缺点**：security-critical kernel WritableRoot 缺口持续打开

---

## 3. Recommendation

**采纳 2C（two-tier），按 ADR-133 §2.5 文件级 slice 先例落地**

### Step C1 — `dasclaw_protocol` 文件级扩展

| 操作 | 来源 | 目标 |
|---|---|---|
| Rename | `crates/dasclaw_parsed_command/` | `crates/dasclaw_protocol/` |
| 保留 | `parse_command.rs` (31 LOC) | 同上 |
| 新增（verbatim 文件级） | `permissions.rs` (2,548 LOC) | `crates/dasclaw_protocol/src/permissions.rs` |
| 新增（verbatim 文件级） | `config_types.rs` (702 LOC) | `crates/dasclaw_protocol/src/config_types.rs` |
| 新增（verbatim 文件级） | `error.rs` (635 LOC) + `error_tests.rs` (547 LOC) | `crates/dasclaw_protocol/src/error.rs` + `error_tests.rs` |
| 新增（verbatim 文件级） | `models.rs` (2,774 LOC) | `crates/dasclaw_protocol/src/models.rs` |
| 新增（verbatim 文件级 + 评估能否独立编译） | `protocol.rs` (5,238 LOC) | `crates/dasclaw_protocol/src/protocol.rs` |
| 新增（verbatim 文件级，配合上） | `network_policy.rs` (22 LOC) | `crates/dasclaw_protocol/src/network_policy.rs` |
| use-path swap | 全部上面文件中的 `codex_utils_absolute_path` → `dasclaw_absolute_path`、`codex_execpolicy` → `dasclaw_execpolicy` | — |

预估 **12,547 LOC verbatim**（70% of full protocol）。

**关键风险**：上述文件可能 use 了 `codex_async_utils` / `codex_utils_image` / `codex_utils_string` / `codex_utils_template` / `codex_network_proxy` 中的类型。如果 use 出现：
- 选项 X — 把那个 utils crate 也 verbatim port 进来（可能再带 +500-1500 LOC）
- 选项 Y — 在 step C1 的"verbatim 文件级 slice"边界用 PR 引言记录的"upstream-pinned-types-only" placeholder（拒绝，违反 verbatim）
- **选项 Z（推荐）** — 把那个 utils crate **小规模 file-level slice** 进 dasclaw_protocol（同样 verbatim 文件级），不新建 crate；step C2 时再分离

### Step C1 PR 拆分（2026-05-XX 修订 — 基于实测依赖闭包）

> **修订原因**：原计划 PR-C1.2=`error.rs` 单独 port 不可行——`error.rs` `use crate::protocol::*` 直接依赖 5,238 LOC `protocol.rs`（原计划 PR-C1.5 才落地），单 PR 编译失败。同样 `config_types.rs` 与 `openai_models.rs` 互相循环依赖，必须同 PR port。原 §3 顺序违反"verbatim 文件级 slice 必须独立编译"原则；保留旧文本会导致实施时被迫 stub 缺失类型 = 补丁式代码。
>
> 实测依赖图详见 §6 Q2 答案。本 §3 修订把 PR 顺序改为**自底向上分层**——每个 PR 内部所有文件互相 cycle-closed，且只 use 已 port 的更下层文件。

**层级图**（箭头 = "use 关系"）：

```
┌────────────────────────────────────────────────────────────┐
│ Layer 4: error.rs / error_tests.rs                         │
│   ↓ uses ThreadId, auth, exec_output, network_policy,      │
│     protocol                                               │
└────────────────────────────────────────────────────────────┘
                    ↓
┌────────────────────────────────────────────────────────────┐
│ Layer 3: protocol / permissions / models / approvals /     │
│          network_policy / items / request_permissions      │
│   ↓ inner cycle: protocol↔permissions↔models               │
└────────────────────────────────────────────────────────────┘
                    ↓
┌────────────────────────────────────────────────────────────┐
│ Layer 2: config_types ↔ openai_models (循环依赖)           │
└────────────────────────────────────────────────────────────┘
                    ↓
┌────────────────────────────────────────────────────────────┐
│ Layer 1: 14 个叶子（zero crate:: deps）                    │
│   account, agent_path, auth, dynamic_tools,                │
│   exec_output(+_tests), mcp, memory_citation,              │
│   message_history, num_format, plan_tool,                  │
│   request_user_input, thread_id, tool_name, user_input     │
└────────────────────────────────────────────────────────────┘
                    ↓
┌────────────────────────────────────────────────────────────┐
│ Layer 0: parse_command.rs (PR #342 已落地)                 │
└────────────────────────────────────────────────────────────┘
```

**修订后的 PR 拆分**：

| PR | 内容 | 文件数 | 估算 LOC | 备注 |
|---|---|---|---|---|
| **PR-C1.1** | rename `dasclaw_parsed_command` → `dasclaw_protocol` + drift guard 拆分 | 14（含 rename）| +217 / -63 | ✅ 已开 [#382](https://github.com/Linnanli/xClaw/pull/382)（in-flight） |
| **PR-C1.2** | Layer 1：14 个叶子文件 verbatim port + lib.rs 出口（`AgentPath` / `ThreadId` / `ToolName`）| 14 + lib.rs + drift guard | ~2,500-3,000 | 单 PR；全 zero `crate::` deps，机械度高 |
| **PR-C1.3** | Layer 2：`config_types.rs` + `openai_models.rs`（互相循环依赖）| 2 + lib.rs + drift guard | ~1,534 | 必须同 PR；解锁 Layer 3 |
| **PR-C1.4** | Layer 3：`protocol.rs` + `permissions.rs` + `models.rs` + `approvals.rs` + `network_policy.rs` + `items.rs` + `request_permissions.rs` | 7 + lib.rs + drift guard | ~12,000 | 最大 PR；inner cycle protocol↔permissions↔models 必须同 PR；**也是 §6 Q2 实测验证点**——这里发现 `codex_async_utils` / `codex_utils_string` / `codex_utils_image` / `codex_network_proxy` 的 use 真实数量并触发选项 Z（utils file-level slice）|
| **PR-C1.5** | Layer 4：`error.rs` + `error_tests.rs` | 2 + lib.rs + drift guard | ~1,182 | 收尾；C1 闭环 |

**总数**：5 个 PR（与原计划相同），但每个 PR 现在都能独立编译。

**每个 PR 仍需验证**：`cargo check -p dasclaw_protocol` 通过 + drift guard 通过 + 旧 use-path 全部 swap 完毕。

**Layer 3 拆分应急方案**（如 reviewer 认为 ~12K LOC 单 PR 太大）：在 PR-C1.4 启动前先评估 `{protocol, permissions, models, request_permissions}` 与 `{approvals, network_policy, items}` 是否可拆为 C1.4a + C1.4b（前者 inner cycle 闭包，后者只 use 前者）。机械可行；视 reviewer bandwidth 决定。

### Step C2 — Full protocol port（W7+）

等 step C1 5 个 PR 全部落地后，再起 ADR-138 评估"是否值得吸收剩余 ~3,500 LOC + 4 transitive utils crate"。届时决策点变成"维护成本 vs codex API 同步速度"。

### 3.1 ADR-129 §1.3 "verbatim 红线" 解读

step C1 的"文件级 slice"是否符合 ADR-129？

✅ **是**，理由：
- `parse_command.rs` 已是文件级 slice 先例（PR #342 + ADR-133 §2.5）
- drift guard 仍然按字节级 SHA 比对每个 vendored 文件
- "切片"不切到类型粒度——`permissions.rs` 整个文件搬过来，里面 50 个类型一个不少
- 与 ADR-129 §1.3 立法目的（"避免我们写一份 codex 写一份长期发散"）一致——只要 drift guard 有效就达标

如 reviewer 反对，备选方案是 2A full crate port（更慢但更纯）。

---

## 4. Out of scope (本 ADR PR)

❌ **不写一行 Rust 代码** — 本 PR 仅落 ADR-136 文档与决策。

❌ 不开 PR-C1.1 / C1.2 / C1.3 / C1.4 / C1.5 实现。

❌ 不修改 `crates/dasclaw_parsed_command/` 任何文件。

❌ 不动 `Cargo.toml` workspace members。

---

## 5. Validation (本 ADR PR)

```bash
python3.12 scripts/check_no_panics.py --base origin/xClaw   # OK (no .rs changed)
python3   scripts/check_no_new_ironclaw_literal.py --base origin/xClaw   # OK
cargo fmt --all -- --check   # OK (no .rs changed)
```

---

## 6. Open questions（留给 reviewer）

1. **Q1 — ADR-129 §1.3 解读**：file-level verbatim slice 是否被 §1.3 接受？还是必须 full-crate? 倾向接受（沿用 ADR-133 §2.5 先例）。
2. **Q2 — `protocol.rs` 5,238 LOC 真的能独立编译吗**：~~未实测~~ **2026-05-XX 部分回答**：实测 `codex-cli-main/codex-rs/protocol/src/` 全 28 文件 `^use ` 行后得：
   - **叶子层（14 文件）零 `crate::` deps**：account/agent_path/auth/dynamic_tools/exec_output/mcp/memory_citation/message_history/num_format/plan_tool/request_user_input/thread_id/tool_name/user_input — 全部仅 use stdlib + 外部 crate（`serde`/`schemars`/`ts_rs`/`uuid`/`thiserror`/`chardetng`/`encoding_rs`/`icu_decimal`/`icu_locale_core`）；其中 `auth.rs` 仅 use `serde + thiserror`。
   - **Layer 2 互相循环**：`config_types.rs` use `crate::openai_models::ReasoningEffort`；`openai_models.rs` use `crate::config_types::{Personality, ReasoningSummary, Verbosity}` → 必须同 PR。
   - **Layer 3 inner cycle**：`protocol.rs` use `crate::config_types::*`；`permissions.rs` use `crate::protocol::*`；`models.rs` use `crate::permissions::*` + `crate::protocol::SandboxPolicy` → 三文件互相依赖必须同 PR。
   - **Layer 4 顶端**：`error.rs` use `crate::{ThreadId, auth, exec_output, network_policy, protocol}`，传递依赖整个 Layer 3 闭包 → 必须最后 port。
   - **transitive utils crate use**（影响选项 Z 触发）：`error.rs` use `codex_async_utils::CancelErr` + `codex_utils_string::truncate_middle_*`；`exec_output.rs` use `chardetng + encoding_rs`（外部 crate 非 codex-utils）；`network_policy.rs` use `codex_network_proxy::*`。`protocol.rs` 5,238 LOC 全 grep 待 PR-C1.4 实施时给最终答案，但目前已知至少需 `codex_async_utils` 与 `codex_utils_string` 各 1 个 file-level slice（选项 Z）。
   - **`codex_network_proxy` use 已确认存在**（`network_policy.rs` 仅 22 LOC）→ ADR-135 Wave-A 阻塞中"#324 sub-task 3 必须先于或同期 port `codex_network_proxy`"被实证。建议在 PR-C1.4 启动前同期开 PR vendor `dasclaw_network_proxy`（small file-level slice，不开新 ADR-138 全 crate port）。
3. **Q3 — codex-protocol 升级节奏**：上游近 6 月 commit log 显示 protocol.rs 平均每周加 1-2 个 enum 变体。drift guard 触发频率会很高 — 是否值得更激进地选 2A full port 来抹平这个？
4. **Q4 — Step C2 时机**：W7 / W8 / W9？建议在 step C1 5 个 PR 全绿后再决定。

---

## 7. Decision log

- **2026-05-08**: 起草，提出 2C two-tier 方案。等待 nally sign-off 决定走 2A 还是 2C。如选 2C，立即起 issue track step C1.1 - C1.5 五个 PR。
- **2026-05-15**: nally 在 epic [#380](https://github.com/Linnanli/xClaw/issues/380) 批准 option 2C two-tier file-level verbatim slice；与 ADR-135 OQ-1 合流。下一步：为 step C1.1 起专属 issue（rename `dasclaw_parsed_command` → `dasclaw_protocol` + drift guard 重命名），作为 Wave-A1 的首个实现 PR。Q2 （`protocol.rs` 是否独立编译）仍作为 step C1.5 启动前验证门，不在本 ADR 预答。
- **2026-05-XX (本 amendment)**: PR-C1.1 实施期间（[#382](https://github.com/Linnanli/xClaw/pull/382)），实测上游 protocol crate `^use ` 行得真实依赖图，发现原 §3 PR 顺序与依赖闭包冲突——`error.rs`（原 C1.2）传递依赖 `protocol.rs`（原 C1.5），`config_types.rs`（原 C1.3）与 `openai_models.rs` 循环依赖。按原顺序硬推会强制 stub 缺失类型 = 补丁式代码。本 amendment 把 §3 PR 拆分改为自底向上 4 层（叶子 → config_types+openai_models → protocol+models+permissions+approvals+items+network_policy+request_permissions → error），每个 PR 内部 cycle-closed 且只 use 下层；§6 Q2 部分回答归档；总 PR 数仍为 5（不变）。next: 起 PR-C1.2 实现 14 个叶子合并 PR。
