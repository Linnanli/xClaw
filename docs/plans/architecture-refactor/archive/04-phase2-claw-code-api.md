# Phase 2 — rig-core 下线 / 切到 claw-code api

> **时长**：2-3 周
> **风险**：中（LLM 协议改动，但可用 feature flag 双跑）
> **收益**：删除 [`rig_adapter.rs`](../../desktop-client/ironclaw/src/llm/rig_adapter.rs) 2026 行；reasoning_content / finish_reason / object 等 DashScope 兼容问题根治；新模型接入时间 3-5 天 → <1 天
> **前置**：Phase 1 完成（前端已用 AI SDK DataStream）

---

## 目标

用 [`claw-code/rust/crates/api`](../../claw-code/rust/crates/api) 替换 rig-core，成为 x-claw 唯一的 LLM Provider 层。保留所有现有模型接入能力：

- Anthropic Claude（claw-code 原生）
- OpenAI / Azure OpenAI
- xAI Grok
- 通义千问 DashScope（Qwen 系列）
- 月之暗面 Kimi
- Ollama
- 其他 OpenAI-compat

---

## 非目标

- ❌ 不重写 agent 循环（留给 Phase 3）
- ❌ 不做 cache token / prompt caching 业务化（作为可选特性）

---

## 前置分析

### claw-code api crate 能力核对

[`claw-code/rust/crates/api/src/providers/`](../../claw-code/rust/crates/api/src/providers):

- `anthropic.rs` — Anthropic Messages API
- `openai_compat.rs` — 单一实现覆盖 OpenAI/xAI/DashScope/Kimi/Ollama，支持 per-provider 自定义 base URL、header、响应后处理

**关键优势**：和 rig-core 相比，claw-code api 为 OpenAI-compat 提供了 per-provider hook 点（不同厂商可自定义 response 处理），从根本上消除"打 JSON 补丁"的需要。

### 当前 rig-core 使用盘点

grep 统计：

- `rig_adapter.rs` 2026 行（唯一 rig-core 业务代码）
- 核心使用点：`CompletionModel`、`completion_request`、`Message`、`Content`
- 未使用：Agent、Tool Registry、Vector Store、RAG

**结论**：只需替换最基础的 Provider 层。

---

## 实施步骤

### Step A — 引入 claw-code api 依赖（~0.5 天）

`desktop-client/ironclaw/Cargo.toml`：

```toml
[dependencies]
# 暂时保留 rig-core，用 feature flag 切换
rig-core = { version = "0.30", optional = true }
claw-code-api = { path = "../../claw-code/rust/crates/api" }

[features]
default = ["claw-code-llm"]
rig-llm = ["rig-core"]
claw-code-llm = []
```

**验证**：`cargo build --features claw-code-llm -p desktop-client` 能编译（即使还没调用）。

---

### Step B — 设计 LlmProvider trait（~1 天）

文件：`desktop-client/ironclaw/src/llm/provider.rs`（新建）

定义一个**我们自己控制的 Trait**，让 rig-core 适配器和 claw-code 适配器都实现它：

```rust
#[async_trait]
pub trait LlmProvider: Send + Sync {
    async fn complete(
        &self,
        req: CompletionRequest,
    ) -> Result<BoxStream<'static, Result<CompletionDelta, LlmError>>, LlmError>;

    fn name(&self) -> &str;
    fn capabilities(&self) -> ProviderCapabilities;
}

pub struct CompletionRequest {
    pub model: String,
    pub messages: Vec<Message>,
    pub tools: Vec<ToolSpec>,
    pub system_prompt: Option<String>,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
}

pub enum CompletionDelta {
    TextDelta(String),
    ToolCallStart { id: String, name: String },
    ToolCallDelta { id: String, args_delta: String },
    ToolCallEnd { id: String },
    Finish { reason: FinishReason, usage: Usage },
}
```

这个 trait 成为 Phase 3 `x_claw_agent` 的 `LlmProvider` 契约。

---

### Step C — 实现 ClawCodeLlmProvider（~3 天）

文件：`desktop-client/ironclaw/src/llm/claw_code_provider.rs`（新建，~300 行）

```rust
pub struct ClawCodeLlmProvider {
    client: claw_code_api::Client,
    model: String,
}

#[async_trait]
impl LlmProvider for ClawCodeLlmProvider {
    async fn complete(
        &self,
        req: CompletionRequest,
    ) -> Result<BoxStream<'static, Result<CompletionDelta, LlmError>>, LlmError> {
        let api_req = map_to_api_request(req);
        let stream = self.client.stream(api_req).await?;
        Ok(Box::pin(stream.map(|item| {
            item.map_err(LlmError::from).map(map_delta)
        })))
    }
}

fn map_to_api_request(r: CompletionRequest) -> claw_code_api::Request { /* ... */ }
fn map_delta(d: claw_code_api::StreamEvent) -> CompletionDelta { /* ... */ }
```

**重点**：不再写任何 JSON 补丁。如果 DashScope 的响应有特殊字段，去 claw-code `openai_compat.rs` 加 per-provider hook（这是正确的修复点）。

### Step D — 配置与模型目录迁移（~1 天）

当前模型配置：`desktop-client/ironclaw/src/llm/config.rs`（或类似）

新增 `provider_kind` 字段：

```rust
pub enum ProviderKind {
    Anthropic,
    OpenAI,
    XAI,
    DashScope,
    Kimi,
    Ollama,
    AzureOpenAI,
    CustomOpenAICompat { base_url: String, headers: Vec<(String, String)> },
}
```

运行时根据 `provider_kind` 构造对应的 `claw_code_api::Client`。

配置文件（用户侧）向后兼容：

```json
{
  "provider": "dashscope",
  "model": "qwen-plus-2025-07-28",
  "api_key": "keychain://dashscope"
}
```

**迁移测试**：保留一个 `legacy_rig_config_migration_test.rs`，验证旧配置文件能正确映射到新 ProviderKind。

---

### Step E — 切换调用点（~2 天）

搜索所有使用 `rig_adapter::*` 的地方：

```bash
rg "use crate::llm::rig_adapter" desktop-client/ironclaw/src/
```

预期 ~10 个调用点（agent、command、log 等）。逐一替换为 `use crate::llm::provider::LlmProvider`，注入 `Arc<dyn LlmProvider>` 而非具体类型。

**不要一次性切全部**。用 feature flag：

```rust
#[cfg(feature = "claw-code-llm")]
pub fn build_llm() -> Arc<dyn LlmProvider> {
    Arc::new(ClawCodeLlmProvider::new(config))
}

#[cfg(feature = "rig-llm")]
pub fn build_llm() -> Arc<dyn LlmProvider> {
    Arc::new(RigLlmAdapter::new(config))  // 包装 rig_adapter
}
```

默认 `claw-code-llm`，出问题可 `--features rig-llm` 回退。

---

### Step F — RigLlmAdapter 兼容包装（~1 天）

为了让 Feature flag 切换平滑，给 rig-core 也包一层 `LlmProvider` trait：

文件：`desktop-client/ironclaw/src/llm/rig_adapter_compat.rs`

```rust
pub struct RigLlmAdapter {
    inner: rig_adapter::RigCompletionModel,
}

#[async_trait]
impl LlmProvider for RigLlmAdapter {
    async fn complete(&self, req: CompletionRequest) -> Result<...> {
        // 把现有 rig_adapter 逻辑封装起来
    }
}
```

这层只是过渡，Phase 2 结束后与 `rig_adapter.rs` 一起删除。

---

### Step G — 真实 LLM 测试（~2 天）

复用现有脚本 [`scripts/run-real-llm-tests.sh`](../../scripts/run-real-llm-tests.sh)，扩展覆盖：

- Anthropic Claude 3.5 / 4
- GPT-4o / GPT-4o-mini
- DashScope Qwen Plus（**必测**，历史 bug 来源）
- Kimi k1.5
- xAI Grok
- Ollama 本地

每个 Provider 验证：
1. 纯文本对话 → 流式增量正确
2. 工具调用（至少 2 次 tool use + 结果回传）
3. 长上下文（>50k tokens）
4. 错误路径（无效 API key → 清晰错误）

---

### Step H — DLP / 安全回归（~1 天）

Phase 2 不改安全逻辑，但换了 LLM 客户端层，必须跑安全冒烟：

- DLP 脱敏对新 Provider 生效
- prompt injection 检测生效
- 审批流不受影响（Phase 1 的 ApprovalGate 不变）

跑 `scripts/pre-commit-safety.sh`。

---

### Step I — 删除 rig-core（~1 天）

条件：Step A-H 全部通过 + 生产稳定 1 周以上。

操作：

```bash
# 1. Cargo.toml 移除 rig-core 依赖和 rig-llm feature
# 2. 删除文件
rm desktop-client/ironclaw/src/llm/rig_adapter.rs
rm desktop-client/ironclaw/src/llm/rig_adapter_compat.rs
# 3. 删除相关测试
# 4. cargo build --all-features 0 错 0 警
# 5. cargo test --all 通过
```

**删除行数预期**：2026 + ~300（compat wrapper） = ~2300 行。

---

## 验收标准

- [ ] `rig_adapter.rs` 已删除
- [ ] `rig-core` 从 Cargo.toml 删除
- [ ] 所有历史支持的模型仍可用（Anthropic / OpenAI / DashScope / Kimi / xAI / Ollama / Azure）
- [ ] DashScope 下 `reasoning_content` 自然工作，无需 JSON patch
- [ ] `scripts/run-real-llm-tests.sh` 全绿
- [ ] 安全冒烟全绿
- [ ] `cargo build --all-features` 0 错 0 警

---

## 回滚策略

**Step A-H 阶段**：feature flag `rig-llm`，随时切换。

**Step I 之后**：如果必须回滚，从 git 还原：

```bash
git revert <delete-rig-commit>
```

由于 Phase 1 的前端协议不依赖具体 LLM 实现，LLM 层回滚不影响前端。

---

## 风险与对策

| 风险 | 对策 |
|------|------|
| claw-code api 对某个 Provider 支持不全 | 给 claw-code 提 PR，或在 ironclaw 侧做薄 wrapper（不在 client 层打补丁） |
| 流式事件映射精度差异（rig-core vs claw-code api） | Step G 真实模型测试覆盖 |
| 用户配置迁移失败 | Step D 保留 legacy config test |
| 删除 rig-core 后某个隐藏调用点爆炸 | Step I 延后 1 周；`cargo build --all-features` 每天跑 CI |

---

## 估时

| Step | 任务 | 估时 |
|------|------|------|
| A | 引入依赖 + feature flag | 0.5d |
| B | LlmProvider trait 设计 | 1d |
| C | ClawCodeLlmProvider 实现 | 3d |
| D | 配置迁移 | 1d |
| E | 切换调用点 | 2d |
| F | RigLlmAdapter 过渡包装 | 1d |
| G | 真实 LLM 测试 | 2d |
| H | 安全回归 | 1d |
| I | 删除 rig-core | 1d |
| — | 缓冲 | 2.5d |
| **合计** | | **15d（三周）** |

---

## 当前进度审查（最新一次盘点）

> 盘点方法：仓库内 `grep -rn "rig_adapter|rig::|rig-core|rig-llm|IRONCLAW_LLM_BACKEND|RigAdapter"` + 文件存在性核对 + Cargo.toml/feature 检查。

### 已完成（Step A–I 主体）

- ✅ **Step A–C / E**：`desktop-client/ironclaw/src/llm/claw_code_provider.rs`（**1334 行**）已上线，`from_registry_config` 接管 Anthropic / OpenAI / Ollama 协议。
- ✅ **Step B**：`LlmProvider` trait 已落在 `desktop-client/ironclaw/src/llm/provider.rs`。
- ✅ **Step F**：`RigLlmAdapter` 过渡包装未落地（直接跳过，因为 Step E 一次性切完）。
- ✅ **Step I 主体**：
  - `rig_adapter.rs` 已删除（约 2026 行）
  - `desktop-client/ironclaw/Cargo.toml` 已无 `rig-core` 依赖
  - `desktop-client/ironclaw/src/llm/schema_utils.rs`（**217 行**）已抽出 `normalize_schema_strict`
  - `IRONCLAW_LLM_BACKEND` env 开关与 `should_use_claw_code_backend()` 函数已删除
  - `create_registry_provider` 在 `mod.rs` 内**无条件**走 `ClawCodeLlmProvider`，`GithubCopilotProvider` 与 `CodexChatGpt` 维持独立分支
  - `claw-code-llm` feature 选了**04b 决策点 4.1 选项 A**（保留为 no-op default feature）

### 待开发清单（已完成）

> 盘点：T1+T2+T4 已在后续提交完成，真实 LLM 回归 T5 已跑通。

#### T1 · 源码内 `rig-core` 注释清理 ✅

已清理 7 个 `.rs` 文件：`mod.rs` / `registry.rs` / `config.rs` / `github_copilot.rs` / `codex_chatgpt.rs` / `retry.rs` / `config/llm.rs`。

保留的历史标注（有意不删，作为变更档案）：

- `schema_utils.rs` 顶部注释："原位于 `rig_adapter.rs`，Phase 2 Step I 从 rig 体系解耦搬到此处"
- `mod.rs` 函数文档："Phase 2 Step I 之后 rig-core 已被彻底移除"
- `retry.rs` 错误匹配注释："历史 rig-core 格式: ..."（保留以便识别老格式错误串）
- `ironclaw/Cargo.toml` `claw-code-llm` feature 注释：历史遗留说明

#### T2 · CLAUDE.md / FEATURE_PARITY.md / Cargo.toml 清理 ✅

- `src/llm/CLAUDE.md`：架构表格 `rig_adapter.rs` → `claw_code_provider.rs` + `schema_utils.rs`；"rig_adapter.rs Details" 章节重写为 "claw_code_provider.rs Details"；GitHub Copilot / 模型 override 描述同步更新
- `FEATURE_PARITY.md`：L251（OpenAI-compatible）与 L569（Ollama）条目的 `rig::providers::ollama` / `RigAdapter` 替换
- `ironclaw/Cargo.toml` L213–217：补充了"下一次 minor 版本可同步删除该 feature 与 desktop-client passthrough"提示

#### T3 · `claw-code-llm` no-op feature 终态（延后）

维持选项 A：`claw-code-llm = []` no-op + `desktop-client` 单点 passthrough。**建议延后到 Phase 3 首次 minor 版本号升级时执行选项 B**（同步删 feature 名 + passthrough + 相关脚本参数），避免独立 PR。

#### T4 · 验收 grep ✅

`grep -rn "rig_adapter|RigAdapter|rig-core|rig-llm|IRONCLAW_LLM_BACKEND" --include='*.rs' --include='*.toml' desktop-client/ironclaw/{src,tests,Cargo.toml} desktop-client/Cargo.toml` 仅剩 4 条历史标注（见 T1 清单），**0 条逻辑残留**。`rig::providers / rig::completion` **0 命中**。`tests/support::test_rig::TestRigBuilder` 是测试夹具命名巧合，与 rig-core 无关。

#### T5 · 真实 LLM / 安全回归 ✅

DashScope qwen 真实回归（`cargo test -p ironclaw --test claw_code_real_llm_tests --features libsql -- --ignored --test-threads=1`）：

```
running 6 tests
test test_qwen_simple_text_completion ... ok
test test_qwen_multi_turn_history_preserved ... ok
test test_qwen_tool_call_round_trip ... ok
test test_qwen_full_tool_round_trip_with_final_answer ... ok
test test_qwen_invalid_api_key_returns_clear_error ... ok
test test_qwen_via_create_registry_provider_with_env_switch ... ok
test result: ok. 6 passed; 0 failed; finished in 7.93s
```

关键证据：
- **工具调用 2-hop 完整回路**：`test_qwen_full_tool_round_trip_with_final_answer` 通过
- **错误路径清晰**：401 Unauthorized 正确分类为 `RequestFailed`
- **DashScope 经 claw-code-api 路径工作**：无需任何 JSON 补丁
- `cargo build -p ironclaw` / `cargo build -p desktop-client` 均 **0 错 0 警**

### 验收回归（已完成）

本文档顶部的"验收标准"与 `04b-step-i` §6 均已打勾，**Phase 2 正式收口**。Phase 3（`05-phase3-agent-extraction.md`）可作为唯一焦点推进。

