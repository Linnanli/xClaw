# ADR-120: Prompt Builder Ownership — 长期归属与迁移路径

- **Status**: Draft（P0-C invariant harness 接受后转 Accepted；见 [#131](https://github.com/Linnanli/xClaw/issues/131)）
- **Date**: 2026-05-04
- **Approver**: x-claw 架构组
- **Issue**: [#89](https://github.com/Linnanli/xClaw/issues/89) — `[P1/W3] Prompt builder ownership ADR`
- **Closes**: #89
- **Source plan**: [`docs/plans/architecture-refactor/48-agent-framework-readiness-and-task-audit.md`](48-agent-framework-readiness-and-task-audit.md) §6.3
- **Related**:
  - [ADR-117 §2 D1](adr-117-p0c-prompt-builder-unification.md) — 已决「吸收到 LayeredPromptBuilder，不新增 crate」（contract 层决策）
  - [ADR-118](adr-118-claw-code-readonly-and-self-impl.md) — claw-code 子仓 read-only + 主仓自实现 LLM provider
  - [ADR-112 §5](adr-112-compatibility-evaluation.md) — W3-A Phase 0 P0-1 boundary 字面量统一（已完成）
  - [#131](https://github.com/Linnanli/xClaw/issues/131) — P0-C invariant harness 实现（hard-dep，本 ADR Accepted 前置）
  - [`p0c-prompt-builder-inventory.md`](p0c-prompt-builder-inventory.md) — 事实底盘

## 修订记 (Revisions)

- **v1.0 (2026-05-04)** — 初稿（Draft）。在 ADR-117 已封顶的 contract 决策之上，进一步定义**crate 级长期归属**。

---

## 1. Context

### 1.1 ADR-117 已经回答了什么

[ADR-117](adr-117-p0c-prompt-builder-unification.md) §2.2 D1–D10 在 W3-A 阶段封顶了**装配 contract**：

- 唯一装配路径 = `LayeredPromptBuilder`（位于 [`desktop-client/ironclaw/src/llm/prompt/`](../../desktop-client/ironclaw/src/llm/prompt/mod.rs)）
- claw-code 的 `SystemPromptBuilder` 不再是装配路径
- 不引入新 crate（拒绝当时讨论的 `dasclaw_prompt` 选项）
- `DynamicLayerInput` 是动态层唯一输入结构体

### 1.2 #89 仍然悬而未决的是什么

#89 issue body 列出 6 个 ADR-必须回答 的问题。其中 5 个已被 ADR-117 / ADR-115 / ADR-118 直接回答，**仅剩 1 个核心问题悬空**：

> Does the canonical prompt contract live in `x_claw_agent::prompt` or a new `dasclaw_prompt` crate/module?

ADR-117 D1 已排除「新建 `dasclaw_prompt`」，但**没有回答**：装配代码长期归属是

- (A) 留在 `desktop-client/ironclaw::llm::prompt`（与 desktop 客户端绑定）
- (B) 上移到 `crates/x_claw_agent::prompt`（与 agent runtime 绑定）

这是本 ADR 唯一新增的决策。

### 1.3 关键事实（三层验证）

`semantic_search` + `vscode_listCodeUsages` + `rg` 三层核验确认：

1. **`crates/x_claw_agent/src/prompt.rs` 当前仅含 1 个 `pub const PROMPT_CACHE_BOUNDARY` 字面量**，文件头明文写着 *「This crate intentionally does not host an assembler trait yet — that lands once the unified implementation is designed.」* — 即从设计起就预留了装配器迁入位点。
2. **`crates/x_claw_agent/src/lib.rs` 头注释**写着 *「Step D: port the runtime loop from ironclaw's `agent/` tree into runtime with hook insertion points. Pending.」* — Phase 3 Step D 的 porting 计划本来就要把 runtime 相关代码（含 prompt 装配）迁入 `x_claw_agent`。
3. **`LayeredPromptBuilder` 的依赖只有 `ToolDefinition` + `PROMPT_CACHE_BOUNDARY`**：前者属 `crate::llm::ToolDefinition`，后者已在 `x_claw_agent::prompt`。装配器迁移**不需要新增跨 crate 边界**，只需把 `ToolDefinition` 一并上移或参数化。
4. **`desktop-client/ironclaw/src/llm/prompt/` 的下游消费方**：`grep` 全仓 `LayeredPromptBuilder`、`DynamicLayerInput`、`StaticLayer` 仅在 `desktop-client/ironclaw/src/llm/` 自身和 `desktop-client/ironclaw/tests/` 出现 — **0 个外部 crate 直接 `use desktop_client_ironclaw::llm::prompt::...`**。迁移半径可控。
5. **codex / claw-code / ironclaw-main 三参考库无对应 crate-边界先例**：codex 的 `Session::build_initial_context` 内联在 session crate，claw-code 的 builder 在 `runtime` crate，legacy ironclaw-main 无 builder。迁移决策属真空地带，由本 ADR 一次性定义。

---

## 2. Decision

### 2.1 总方针

**长期归属 = `crates/x_claw_agent::prompt`**。

`LayeredPromptBuilder`、`StaticLayer`、`DynamicLayerInput`、`StaticLayerConfig` 在 P0-C invariant harness（[#131](https://github.com/Linnanli/xClaw/issues/131)）落地后，整体迁入 `crates/x_claw_agent/src/prompt/`，模块结构与现状一一对应：

```
crates/x_claw_agent/src/prompt/
├── mod.rs              ← LayeredPrompt / LayeredPromptBuilder + cache_boundary_section
├── static_layer.rs     ← StaticLayer + StaticLayerConfig
├── dynamic_layer.rs    ← DynamicLayerInput
└── boundary.rs         ← 现有 PROMPT_CACHE_BOUNDARY（从 prompt.rs 拆出来）
```

`desktop-client/ironclaw::llm::prompt` 改为薄 re-export 适配层（仅 6–8 行），保 desktop-client 一年内的旧 import 路径兼容。

### 2.2 五项细决策（O1–O5）

| # | 决策 | 选项 | 备注 |
|---|---|---|---|
| **O1** | crate 长期归属 | **B — `x_claw_agent::prompt`** | 与 Phase 3 Step D agent runtime 迁移路径对齐；prompt 装配是 agent runtime 责任，不是桌面客户端责任 |
| **O2** | 是否新增 `dasclaw_prompt` crate | **否** | 复述 ADR-117 D1 — 不新增空 crate；所有 prompt 资产集中在 `x_claw_agent::prompt` 模块 |
| **O3** | `desktop-client/ironclaw::llm::prompt` 处置 | **降级为 re-export 适配** | `pub use x_claw_agent::prompt::{LayeredPromptBuilder, StaticLayer, ...};` 保 desktop-client 当前 21 处 import 不破；下一波（W6+）按 import 数自然清零节奏移除 |
| **O4** | `ToolDefinition` 边界 | **不上移到 `x_claw_agent`** | `ToolDefinition` 留在 `desktop-client/ironclaw::llm`；改成把 `LayeredPromptBuilder::new` 的 `tools` 参数从 `&[ToolDefinition]` 抽象为 `&[impl ToolSpec]` trait（最小 trait：`name() / description() / parameters_schema()`），由 `desktop-client/ironclaw::llm::ToolDefinition` 实现 |
| **O5** | 迁移触发门 | **P0-C invariant harness 接受后** | 不在 harness 之前迁，避免 byte-level prefix 稳定性回归；harness PR 合入后启动迁移 PR |

### 2.3 三层组合优先级（保持 ADR-117 D8 决策）

为避免 #89 验收第 5 项重复发明：ProjectDocs / skills / admin policy / output style / tool metadata / language preference 的组合顺序**完全沿用 [ADR-117 §2.2 D8.x](adr-117-p0c-prompt-builder-unification.md#22-十项细决策d1d10)**，本 ADR 不做任何变更。

---

## 3. 迁移序列（Migration Sequence）

| 步 | 工作项 | 依赖 | 估时 |
|---|---|---|---|
| **M1** | P0-C invariant harness 落地（[#131](https://github.com/Linnanli/xClaw/issues/131)） | — | hard dep，外部完成 |
| **M2** | 引入 `ToolSpec` trait（O4），`LayeredPromptBuilder::new` 改 `&[impl ToolSpec]` | M1 | 0.5d |
| **M3** | 整体 `git mv desktop-client/ironclaw/src/llm/prompt/* crates/x_claw_agent/src/prompt/`，调整 `mod.rs` 树 | M2 | 0.5d |
| **M4** | `desktop-client/ironclaw::llm::prompt` 改为 `pub use x_claw_agent::prompt::*` 适配层 | M3 | 0.2d |
| **M5** | P0-C harness 重跑，验证 byte-level prefix 稳定性零回归 | M3 | harness 自带 |
| **M6** | （W6+ 自然清零）随 import 重写自然移除 desktop-client 适配层 | M4 | non-blocking |

迁移用 1 个 PR 完成 M2–M5，M6 留给后续随手清。

---

## 4. Non-Goals

本 ADR **不**：

- ❌ 不重新讨论 ADR-117 已封顶的装配 contract（D1–D10 全保留）
- ❌ 不新建 `dasclaw_prompt` 或 `prompt_contract` crate
- ❌ 不改 `PROMPT_CACHE_BOUNDARY` 字面量（[ADR-112 §5](adr-112-compatibility-evaluation.md) 已锁）
- ❌ 不在 P0-C harness 接受前做迁移（O5）
- ❌ 不强制 codex/claw-code/ironclaw-main 三参考库 follow（fork 各自决策）
- ❌ 不引入 prompt 多 provider 抽象（Anthropic cache_control 切分等留给后续 P1，见 ADR-117 §6 R-1）

---

## 5. Acceptance Tests（迁移 PR 要求）

迁移 PR 必须满足：

1. **P0-C invariant harness 全绿**：byte-level prefix 稳定性测试零回归。
2. **desktop-client/ironclaw 测试集全绿**：`cargo nextest run -p ironclaw` 及相关 prompt 单测/集成测试零失败。
3. **编译路径验证**：`cargo check -p x_claw_agent --tests` + `cargo check -p ironclaw --tests` 双 0 错 0 警。
4. **Re-export 兼容性**：迁移前后 `desktop-client/ironclaw` 内 21 处 `use crate::llm::prompt::...` import **不需要修改**。
5. **三层验证留痕**：commit message 含 `已检查 X 是否已有，结论：…`（按 [AGENTS.md](../../../AGENTS.md) 「分析工具使用规范」 落实）。
6. **零新 panic**：`python3.12 scripts/check_no_panics.py --base origin/xClaw` 通过。

---

## 6. Risks & Mitigations

| 编号 | 风险 | 缓解 |
|------|------|------|
| **R-1** | `ToolSpec` trait 抽象不足，下游需要新方法 | trait 设计预留 `#[non_exhaustive]` + `default fn` 钩子；首期只暴露 `name/description/parameters_schema`，下游真要扩展再加 |
| **R-2** | git mv 跨 crate 移文件时 IDE / sccache 缓存失效，本地构建变慢 | 迁移 PR 单独跑一次 `cargo sweep --time 3` 即可；CI 不受影响 |
| **R-3** | `desktop-client/ironclaw::llm::prompt` 适配层永久遗留 | M6 工作项跟踪 import 数量，归零后 follow-up PR 删除适配层 |
| **R-4** | `x_claw_agent` crate 编译时长上升 | 当前 `x_claw_agent` 是 `desktop-client/ironclaw` 的依赖；prompt 装配代码挪上去后实际反而减少 desktop-client 重编译范围（agent crate 改动频率更低） |
| **R-5** | 与 W3 / W4 其他 prompt-相关 PR 冲突 | 等 P0-C #131 合入再启动 M2，避免 stacked 冲突 |

---

## 7. 与 codex / claw-code / ironclaw-main 的对比（Reference）

| 项目 | prompt 装配位置 | 与本 ADR 对比 |
|---|---|---|
| `claw-code` | `runtime` crate `SystemPromptBuilder` | runtime crate 是 agent runtime crate；与本 ADR O1 同向（agent runtime 持有） |
| `codex-cli-main` | `Session::build_initial_context` 内联 session crate | session crate 也是 runtime 相关；与本 ADR O1 同向 |
| `ironclaw-main`（legacy） | `Workspace::system_prompt()` 内联 server | 不参考（legacy） |

三参考库均把 prompt 装配放在「runtime / session 类 crate」而非「客户端 / 应用层 crate」，本 ADR O1 选 `x_claw_agent` 与该惯例一致。

---

## 8. 决策日志（Decision Log）

| 日期 | 事件 |
|------|------|
| 2026-05-04 | v1.0 草拟，Status: Draft，等待 [#131](https://github.com/Linnanli/xClaw/issues/131) P0-C harness 落地后转 Accepted |
