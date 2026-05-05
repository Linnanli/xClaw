# ADR-117: P0-C Prompt Builder 统一（吸收 claw-code 至 LayeredPromptBuilder）

- **Status**: Accepted (v1.3 partial-supersede)
- **Date**: 2026-04-30
- **Last revision**: 2026-05-05 (v1.3)
- **Approver**: x-claw 架构组
- **Issue**: [#130](https://github.com/Linnanli/xClaw/issues/130)（W3/B P0-C — Prompt Builder 统一） — closed by [#142](https://github.com/Linnanli/xClaw/pull/142)；[#131](https://github.com/Linnanli/xClaw/issues/131)（W3/C 实施）— closed by [#141](https://github.com/Linnanli/xClaw/pull/141) + [#142](https://github.com/Linnanli/xClaw/pull/142)（D5/D7 撤回，详 v1.3 修订记）
- **Closes**: #130, #131

## 修订记 (Revisions)

- **v1.0 (2026-04-30)** — 初稿（Proposed）
- **v1.1 (2026-05-02, 已撤回)** — 试图扩档 D7 删除半径至「档 1.5」（在 claw-code 子仓内删 5 项 crate），方向错误，PR #143 已 close
- **v1.2 (2026-05-02)** — **D5 + D7 子仓内删除工作项已撤回**（claw-code 子仓恢复原代码 db8ff4d revert c36c0ef），由 [ADR-118](adr-118-claw-code-readonly-and-self-impl.md) 取代为「主仓自实现 LLM provider + 删除 desktop-client/ironclaw 对 claw-code-api 的 path-dep」。本 ADR 其余部分（D6 / D8.2 / D8.3 / D8.4 / 提示词单源核心目标）保持有效。
- **v1.3 (2026-05-05)** — **W3/C 实施 issue [#131](https://github.com/Linnanli/xClaw/issues/131) 收尾**：三层验证（`semantic_search` + `grep_search`）确认 `SystemPromptBuilder` 在 x-claw 主仓代码层 0 调用方（仅在只读区 `claw-code/rust/crates/runtime/` 与本 ADR 注释中出现）；`DynamicLayerInput.environment` / `project_doc` 字段、`## Environment` Markdown render、reasoning.rs 喂数据、5 个不变量测试均已在 #141 + #142 落地；D5/D7 已由 v1.2 + ADR-118 接手。结论：**#131 的 7 个 scope checkbox 全部由 #141 + #142 + ADR-118 履约**，无新增代码改动需要落地。本 v1.3 修订仅为关闭 issue 的过程透明度记录。

### Done 状态（v1.2 截至 2026-05-02）

| 工作项 | PR | 状态 |
|---|---|---|
| ADR doc 起草 | [#140](https://github.com/Linnanli/xClaw/pull/140) | ✅ Merged ca99622 |
| D8.2 + D8.3 cache refactor | [#141](https://github.com/Linnanli/xClaw/pull/141) | ✅ Merged b8049f4c |
| D6 + D8.4 删 static_hash / legacy 字段 | [#142](https://github.com/Linnanli/xClaw/pull/142) | ✅ Merged 51e14910，closes #130 |
| **核心目标「提示词单源 = LayeredPromptBuilder」** | — | ✅ **达成** |
| **W3/C 实施 #131 收尾**（v1.3） | 本文 doc-only PR | ✅ closes #131（7 scope items 全部履约：见 v1.3 修订记） |
| ~~v1.1 D7 扩档（在子仓内删）~~ | ~~[#143](https://github.com/Linnanli/xClaw/pull/143)~~ | ❌ Closed（方向错误，由 ADR-118 取代） |
| ~~父仓 bump submodule~~ | ~~[#144](https://github.com/Linnanli/xClaw/pull/144)~~ | ❌ Closed（子仓 revert 后无需 bump） |
| **D5 + D7 撤回** | [ADR-118](adr-118-claw-code-readonly-and-self-impl.md) | 🟡 Pending（W6 实施） |
- **Related**:
  - [p0c-prompt-builder-inventory.md](p0c-prompt-builder-inventory.md)（事实底盘 + §11 16 个待决问题，本 ADR 是其封顶）
  - [adr-112-compatibility-evaluation.md §5](adr-112-compatibility-evaluation.md)（W3-A Phase 0 P0-1 boundary literal 统一前置）
  - [adr-113-hook-engine-unification.md §2.4](adr-113-hook-engine-unification.md)（反跨界原则 — 解释为什么 ProjectDoc 不走 hook）
  - [adr-115-project-docs-not-in-hook.md](adr-115-project-docs-not-in-hook.md)（构建期 DI / project_doc 字段对齐）
  - [openai-cache-augmentation-issue-draft.md](openai-cache-augmentation-issue-draft.md)（P1 后续，不阻塞本 ADR）
  - 未来 P1 issue：Anthropic provider 实现 cache_control 切分（本 ADR §6 R-1 风险条目登记）
- **Numbering note**: 仓库 inventory + draft 早期文件提到的「ADR-114 prompt builder 统一」实际编号让位至 117（114 已被 [adr-114-dasclaw-rebrand.md](adr-114-dasclaw-rebrand.md) 占用，115/116 也已分别预留）。同步将 `openai-cache-augmentation-issue-draft.md` 中 3 处「ADR-114」更正为「ADR-117」。

---

## 1. Context

### 1.1 三参考库 + 一 fork 的 prompt 装配现状

依据 [p0c-prompt-builder-inventory.md](p0c-prompt-builder-inventory.md) §1–§3：

| 项目 | 装配入口 | 模式 | 装配产物 |
|---|---|---|---|
| `claw-code` | `SystemPromptBuilder` + `load_system_prompt`（[runtime/src/prompt.rs](../../claw-code/rust/crates/runtime/src/prompt.rs)） | 9 段构建器，永远 emit `__SYSTEM_PROMPT_DYNAMIC_BOUNDARY__` 裸字面量 | `Vec<String>` |
| `desktop-client/ironclaw` (本仓库 fork) | `LayeredPromptBuilder`（[desktop-client/ironclaw/src/llm/prompt/mod.rs](../../desktop-client/ironclaw/src/llm/prompt/mod.rs)） | 静态/动态二层 + `static_hash: u64` + `with_cache_boundary(is_anthropic)` 条件 emit `<!-- {boundary} -->` HTML 注释 | `String` |
| `codex-cli-main` | `Session::build_initial_context`（无独立 builder type） | inline composition：top-level `instructions` API 字段 + developer/contextual_user 多 message item | typed `Vec<TurnContextItem>` |
| `ironclaw-main`（legacy server） | `Workspace::system_prompt()` 不透明字符串 + 64KB HTTP cap | 无 builder | `String` |

W3-A Phase 0 P0-1（[ADR-112 §5](adr-112-compatibility-evaluation.md)）已统一**字面量**（`x_claw_agent::PROMPT_CACHE_BOUNDARY`），但**装配代码**仍在两处并行：claw-code 的 `SystemPromptBuilder` 与 fork 的 `LayeredPromptBuilder`。

### 1.2 #130 / W3/B 验收要求

issue #130 要求 W3/B 阶段**统一 prompt builder 装配路径**，作为 ADR-101 吸收式重构的 P0-C 节点。判定材料 = inventory §11 的 16 个待决问题。

### 1.3 三层验证已完成的关键负向断言

inventory §10 + 本 ADR 起草过程的三层验证（semantic_search → vscode_listCodeUsages → grep）确认：

1. **boundary marker 在所有 provider 端无消费方**：grep 全仓 `cache_control` 在 desktop-client/ironclaw、claw-code、ironclaw-main 的 provider 适配层 0 命中。当前 marker 的实际行为是 **inert HTML comment**，Anthropic prompt cache 命中率 = 0。
2. **`DynamicLayerInput.admin_policy` / `token_budget` / `language_preference` 是 0 caller 死字段**：`git log -S` 确认仅一次 P0-1 commit（3f45d54）引入；`vscode_listCodeUsages` + `rg` 全仓 0 调用方。三参考库（codex / claw-code / ironclaw-main）均无对应概念。
3. **codex 装配模型与 ironclaw 不可融合**：codex 走 Responses API 顶层 `instructions` 字段 + developer/contextual_user 多消息项，无 inline boundary；不能直接借鉴，但可作为未来 P1 优化方向。
4. **inventory 35/36/37 三份 capability inventory 均未拍 composition 顺序**：D8 决策属真空地带，由本 ADR 一次性定义。
5. **`claw-code-api` 依赖 `claw-code/rust/crates/runtime` 仅用于 OAuth/usage/config**，与 `runtime::prompt` 模块无耦合 → 删除 `runtime/src/prompt.rs` 不破 desktop-client 编译。
6. **`claw-code/rust/crates/rusty-claude-cli` 是 desktop-client 的死代码**：grep workflows + Cargo.toml + lockfile 0 命中。

---

## 2. Decision

### 2.1 总方针

**保留 desktop-client/ironclaw 的 `LayeredPromptBuilder` 为唯一装配路径**；**删除 claw-code 的 `SystemPromptBuilder`**；其需要的能力（环境段、project doc 段）以**字段化**形式吸收进 `DynamicLayerInput`，**不引入新模块**。

### 2.2 十项细决策（D1–D10）

| # | 决策 | 选项 | 备注 |
|---|---|---|---|
| **D1** | 统一目标 | **B — 吸收到 LayeredPromptBuilder** | 不另起新 crate；与 ADR-101 吸收式重构总方针一致 |
| **D2** | boundary 字面量来源 | **B' — 通过 `x_claw_agent::PROMPT_CACHE_BOUNDARY` 单源** | claw-code 端的同名常量随 D5 删除一并消失 |
| **D3** | 吸收范围 | **Absorb a/b/c/d/e/g/i** | inventory §11 的待决问题 a/b/c/d/e/g/i 由本 ADR 给出最终选择 |
| **D4** | `IRONCLAW_PROMPT_LAYERING` 环境变量 | **C — 删除** | P0-1 已默认启用并删除；本 ADR 仅追认 |
| **D5** | `claw-code/rust/crates/runtime/src/prompt.rs` | ~~drop — 整文件删除~~ → **撤回 (v1.2)**：保留子仓代码不动，由 [ADR-118](adr-118-claw-code-readonly-and-self-impl.md) 取代为「主仓自实现 + 删 path-dep」 | desktop-client 不依赖 `runtime::prompt`；claw-code 子仓改为只读参考库 |
| **D6** | `LayeredPromptBuilder.static_hash` + `static_changed` 旗标 | **Y — 删除** | 该 hash 字段无消费者，prefix cache 真正依赖的是字节稳定性，而非业务层 hash 比较 |
| **D7** | 删除半径 | ~~档 1 保守 — 仅删 `runtime/src/prompt.rs` + `rusty-claude-cli/`~~ → **撤回 (v1.2)**：claw-code 子仓整体保持原貌，由 [ADR-118](adr-118-claw-code-readonly-and-self-impl.md) 取代为「主仓自实现 LLM provider + 删 desktop-client/ironclaw 对 claw-code-api 的 path-dep」 | 子仓代码 0 行变化；路径变为主仓 `crates/dasclaw_llm_provider` 新建 |
| **D8** | composition 顺序 | 见 §2.3（6 个子决策） | inventory 35/36/37 真空地带，本 ADR 一次性拍板 |
| **D9** | 测试不变量 | 见 §2.4（5 个子决策） | 删 3 加 5；不引入快照测试；不设覆盖率门槛 |
| **D10** | 落地序列 | **三段 PR** + 立即删除 + 单 commit revert | 见 §4 |

### 2.3 D8 — composition 顺序（6 子决策）

| # | 子决策 | 取值 | 理由 |
|---|---|---|---|
| **D8.1** | identity（workspace AGENTS.md / SOUL.md / IDENTITY.md）所属层 | **静态层尾部 `--- {identity}`** 不变；项目级文档以 `DynamicLayerInput.project_doc` **占位字段**迎接 [#55 ProjectDocLoader](https://github.com/Linnanli/xClaw/issues/55)（已 closed）的产出 | identity 是「我是谁」恒定身份；project_doc 是「项目的指令」按 cwd 变化 — 两者生命周期不同 |
| **D8.2** | 新增 `## Environment` 段格式 | **Markdown**：`## Environment\n\ncwd: {path}\ndate: {date}\nplatform: {os}` | 维持 dynamic 层全 Markdown，避免 codex 的 XML（`<environment_context>`）与 Markdown 混排 |
| **D8.3** | `project_doc` 字段是否本 ADR 立即接入 ProjectDocLoader | **A — 暂不接 + 字段预留** | [#55 ProjectDocLoader](https://github.com/Linnanli/xClaw/issues/55) 本体已 closed（loader 逻辑实现完），但接入 `LayeredPromptBuilder` 的字段赋值工作属后续 issue；本 ADR 仅预留 `Option<String>` 接口面，避免将来反复改 schema |
| **D8.4** | `admin_policy` / `token_budget` / `language_preference` 三字段 | **B — 删除** | 三层验证证实 0 caller、0 上游对应、P0-1 死占位；未来正确路径已分别登记到 [#137](https://github.com/Linnanli/xClaw/issues/137) |
| **D8.5** | 静态层内部顺序 | **保持现状** | preamble → response_format → guidelines → safety → tools → tool_style → identity；inventory §3.1 已稳定 |
| **D8.6** | 动态层内部顺序 | **保持现状基础上插入新段** | skill → channel → ext → conv → group → runtime → **environment（新）** → **project_doc（新占位）** |

#### 2.3.1 最终 `DynamicLayerInput` 字段形态

```rust
pub struct DynamicLayerInput {
    pub skill_context: Option<String>,
    pub channel: Option<String>,
    pub extensions_guidance: Option<String>,
    pub conversation_context: Option<String>,
    pub group_guidance: Option<String>,
    pub runtime_info: Option<String>,
    pub environment: Option<String>,    // D8.2 新增
    pub project_doc: Option<String>,    // D8.3 占位（#55 loader 已 closed；接入字段赋值是后续 issue）
}
```

删除字段：`admin_policy` / `token_budget` / `language_preference`（D8.4）。

### 2.4 D9 — 测试不变量（5 子决策）

| # | 子决策 | 取值 |
|---|---|---|
| **D9.1** | 删除测试 | `test_admin_policy_precedence_note`（dynamic_layer.rs）+ `test_static_hash_*` 系列（mod.rs）共 3 项 |
| **D9.2** | 新增功能测试 | `test_environment_section_renders`（验证 `## Environment` Markdown 正确组装） + `test_project_doc_field_default_empty`（占位字段在 `None` 时不 emit） |
| **D9.3** | 新增 provider 通用性测试 | `test_openai_compat_prompt_serializes` + `test_anthropic_compat_prompt_serializes` —— 锁定「inert HTML comment 在所有 provider 都是合法静态字节，不破坏序列化」 |
| **D9.4** | 顺序断言形式 | **B — `text.find()` 相对位置断言**（不引入 `insta` 快照）。理由：prompt 文本会随产品迭代频繁修订，快照会变成「狼来了」噪声；位置断言锁的是不变量本身（`environment 早于 project_doc`），不锁文案 |
| **D9.5** | 覆盖率门槛 | **C — 不设** | x-claw 已有 TDD + Rust 类型 + 多层 CI gate 作为强约束；覆盖率门槛是弱代理（Goodhart 风险），不写 |

净测试变化：**+5 / -3**。

---

## 3. Consequences

### 3.1 正面

- 装配路径单源化：`Reasoning::build_system_prompt_with_tools` → `LayeredPromptBuilder::build` → `String`，可被任意 provider 透传
- `DynamicLayerInput` 形态契合 codex `EnvironmentContext` 概念（cwd/date/platform），未来若接 #55 ProjectDocLoader 不需要再改 schema
- 删除 ~600 LOC 死代码（runtime/src/prompt.rs ~920 行 + rusty-claude-cli + 3 个 dynamic 字段 + hash 系列）
- ADR-115 的「构建期 DI」原则得到字段化承接（`project_doc: Option<String>`）

### 3.2 负面 / 成本

- 删除 `claw-code/rust/crates/runtime/src/prompt.rs` 后，`claw-code` 子项目的 standalone 用途（如果有）失去 system prompt 装配能力 → 但本仓库不依赖该路径，可接受
- 跨 crate 删除 PR（PR-3）需要 `cargo check -p claw-code-api` 双重验证

### 3.3 风险

| # | 风险 | 缓解 |
|---|---|---|
| **R-1** | **boundary marker 全 provider 无消费**（核心遗留问题） | 本 ADR **不在范围内**修复 — 单独 P1 issue「Anthropic provider 接入 `cache_control` 切分」；本 ADR §1.3 已记录该负向事实底盘 |
| **R-2** | PR-3 删 `rusty-claude-cli` 破 CI workflow | PR-3 必须先 `grep -r "rusty-claude-cli" .github/workflows/` 0 命中 |
| **R-3** | PR-3 删 `runtime/src/prompt.rs` 破 `claw-code-api` 编译 | PR-3 必须 `cargo check -p claw-code-api` 通过 |
| **R-4** | `## Environment` 段格式与 #55 ProjectDocLoader 将来要求冲突 | `project_doc` 字段已预留；#55 实施时仅填字段，不改格式 |
| **R-5** | `admin_policy` / `token_budget` / `language_preference` 误删（万一上游 fork 或某 milestone 真要用） | 已三层验证 0 caller，且未来正确路径登记到 [#137](https://github.com/Linnanli/xClaw/issues/137)（admin_policy → ironclaw_safety fail-safe / token_budget → context/compact / language_preference → ChannelAdapter）；删除是字段重构，不是能力删除 |

---

## 4. 落地序列（D10）

### 4.1 三段 PR

| PR | 标题 | base | closes | 内容 | LOC | 风险 |
|---|---|---|---|---|---|---|
| **PR-1** | `docs(adr): adr-117 p0c prompt builder unification` | xClaw | — | 仅本文档 | +~400 / -0 | 0 |
| **PR-2** | `feat(prompt): implement adr-117 d8 composition order` | PR-1 合并后 xClaw | — | `DynamicLayerInput.environment` + `project_doc` 字段；`dynamic_layer.rs` render；`reasoning.rs` 喂 `EnvironmentContext::current()` 数据；新增 5 测试 | +~80 / -5 | 低 |
| **PR-3** | `chore(prompt): remove dead code per adr-117 d5/d6/d7/d8.4` | PR-2 合并后 xClaw | **#130** | 删 `admin_policy`/`token_budget`/`language_preference` 字段 + `dynamic_layer.rs` 对应 render；删 `LayeredPromptBuilder.static_hash` + `static_changed` + hash 测试；删 `claw-code/rust/crates/runtime/src/prompt.rs`（D5）；删 `claw-code/rust/crates/rusty-claude-cli/`（D7）；同步 `runtime/lib.rs` 的 `pub use` | +~10 / -~600 | 中 |

### 4.2 每 PR 必跑验证

依据 [.github/copilot-instructions.md](../../.github/copilot-instructions.md) 第 3 节本地管线：

```bash
cargo check -p ironclaw --tests
cargo nextest run -p ironclaw --lib llm::prompt
cargo fmt --all
python3.12 scripts/check_no_panics.py --base origin/xClaw
cargo clippy --no-deps -p ironclaw --all-targets -- -D warnings
```

PR-3 额外：

```bash
cargo check -p claw-code-api                                   # 不破依赖
grep -r "rusty_claude_cli\|runtime::prompt" --include='*.rs'   # 无残留引用
grep -r "rusty-claude-cli" .github/workflows/                  # 无 workflow 引用
```

### 4.3 删除时机

**立即删除**，无 deprecate 期：三层验证已确认 0 caller，无外部 fork 依赖路径需要给迁移窗口。

### 4.4 回滚策略

**单 commit revert**。不引入 `IRONCLAW_PROMPT_V2` 之类 feature flag——feature flag 即「补丁式代码」，违反 [AGENTS.md](../../AGENTS.md) 红线。

---

## 5. Enforcement

### 5.1 PR body 必填项

按 [.github/copilot-instructions.md](../../.github/copilot-instructions.md) 第 4 节：每个 PR body 必须包含 4 块（背景/目标 / 改动范围 / What's NOT in this PR / 验证）。stacked PR（PR-2 / PR-3）必须额外注明 base 不是 `xClaw` 与 merge 顺序。

### 5.2 范围外（What's NOT in this PR）

- **不修复 boundary marker 无 provider 消费**（R-1，单独 P1 issue）
- **不接入 #55 ProjectDocLoader**（仅占位字段）
- **不删除整个 `claw-code` 子树**（v1.2 进一步强化：子仓代码 0 行变化，作为只读参考库；原 D7 「档 1 保守」措辞已撤回，详 [ADR-118](adr-118-claw-code-readonly-and-self-impl.md)）
- **不重新设计 codex-style top-level `instructions` 装配**（与本仓库 ChatMessage::system 单串模型不兼容，留待后续 ADR）
- **不引入 `insta` 快照测试**（D9.4）
- **不设覆盖率门槛**（D9.5）

---

## 6. 验证

### 6.1 三层验证账本

inventory [§10](p0c-prompt-builder-inventory.md) + 本 ADR 起草过程已完成：

- **Tier 1 semantic_search**：定位 `LayeredPromptBuilder` / `SystemPromptBuilder` / `Session::build_initial_context` 三处装配
- **Tier 2 vscode_listCodeUsages**：`admin_policy` / `token_budget` / `language_preference` 0 调用方；`runtime::prompt` 在 desktop-client 0 调用方
- **Tier 3 grep_search / rg**：
  - `cache_control` 在 desktop-client/ironclaw、claw-code、ironclaw-main provider 层 0 命中（R-1 事实底盘）
  - `git log -S` 确认 admin_policy 等三字段仅 P0-1 commit（3f45d54）引入
  - `rusty-claude-cli` 在 workflows / Cargo / lockfile 0 命中

### 6.2 关联 issue

- **#130**：本 ADR 实施 PR-3 closes
- **#55**：[CLOSED] ProjectDocLoader loader 实现本体已完；将其输出接入 `DynamicLayerInput.project_doc` 字段赋值属后续 issue
- **#137**：admin_policy / token_budget / language_preference 三字段未来正确路径追踪
- **未来 P1（待开 issue）**：Anthropic provider 实现 `cache_control` 切分（R-1）

---

## 7. 关键术语

| 术语 | 含义 |
|---|---|
| **静态层** | 与 cwd / 用户输入无关、跨 session 字节稳定的前缀（identity / response_format / guidelines / safety / tools） |
| **动态层** | 跨 session 变化的后缀（skill / channel / runtime / environment / project_doc） |
| **boundary marker** | `<!-- __SYSTEM_PROMPT_DYNAMIC_BOUNDARY__ -->`，HTML 注释包裹的字面量，理论用途为辅助 provider 端做 prefix cache 切分；当前**实际为 inert HTML 注释**（无 provider 消费） |
| **inert** | 模型可读但语义中立——HTML 注释对所有主流 LLM provider 都是合法静态字节 |
| **provider 通用性** | 同一 prompt 串可作为 `ChatMessage::system` 内容透传给 OpenAI / Anthropic / Gemini / Bedrock / Copilot 等任意 provider 而不破坏其请求 schema |
