# ADR-158：LLM runtime 热替换抽象的协议层补 port + desktop 薄壳重构

- 状态：Accepted（随 PR #1045 + PR #1046 落地）
- 提案人：Coding Agent
- 关联：ADR-118（claw-code 只读化 + LLM provider 中立迁出）、ADR-129（verbatim port mandate）、F3.1（PR #627 / #637 LLM port to dasclaw_llm_provider）
- 时间：2026-05-31

---

## 1. 背景

ADR-118 §1.4 三层验证已经说明：`desktop-client/ironclaw/src/llm/` 与 `ironclaw-main/src/llm/` 共同祖先，但 fork 后 desktop 走了「自实现 claw-code-api」路线，ironclaw-main 留在「rig-core 适配」路线。F3.1（PR #627 / #637）启动了第二阶段统一：把 desktop 现有的协议层文件 **port 到 `crates/dasclaw_llm_provider/src/provider/`**，让所有协议层抽象成为可被无头 agent 框架复用的中立 crate。

F3.1 commit message 字面写：

> `7e76eb963 WIP(F3.1): port ironclaw/src/llm → dasclaw_llm_provider/provider [#627]`

实际 port 了 13 个文件（bedrock / failover / smart_routing / retry / circuit_breaker / response_cache / token_refreshing / costs / config / error / image_models / models / registry 等）。但 `ironclaw-main/src/llm/` 下还有 7 个文件**未被 port**，且 `dasclaw_llm_provider/src/lib.rs` 顶部注释只解释了「不复刻 pricing 耦合」，对其它 6 个的取舍没有留下任何说明。

2026-05-31 一次 desktop 引擎启动失败（`ENGINE_START_FAILED: Authentication failed for provider openai`）触发了重新审视，发现：

1. 真正的 bug 根因不是 F3.1，而是 `apply_admin_overrides` 用 `env::set_var` 无脑覆盖 LLM_BACKEND
2. 但顺藤摸瓜发现，desktop 的 `ModelSwitchProvider`（128 LOC，`Mutex<Arc<dyn LlmProvider>>` 简化版）是**平行重发明**——`ironclaw-main/src/llm/runtime.rs` 早就有更成熟的 `SwappableLlmProvider`（570 LOC，`RwLock<ProviderSnapshot>` + metadata 缓存 + `LlmReloadHandle` 并发 serialize），但被 F3.1 跳过了

## 2. 7 个 F3.1 跳过文件的真实分类

经 4 轮三层验证（semantic_search + grep + read_file + cross_repo_search）确认：

| # | 文件 | LOC | 当前 x-claw 状态 | 真实分类 | 处置 |
|---|------|----:|------------------|----------|------|
| 1 | `runtime.rs` (`SwappableLlmProvider` + `LlmReloadHandle`) | 570 | x-claw 0 引用，无替代 | 协议层真空白 | **应补 port** (本 ADR 推动) |
| 2 | `session.rs` (NEAR AI OAuth token) | 912 | 已在 `desktop-client/ironclaw/src/llm/session.rs` | 应用层（依赖 db / secrets，已正确分层） | 维持现状 |
| 3 | `recording.rs` (HTTP 录制 / `RecordingLlm`) | 1962 | 已在 `desktop-client/ironclaw/src/llm/recording.rs`；底层 `HttpInterceptor` 已下沉 `dasclaw_runtime::recording` | 应用层 + 协议层已分离 | 维持现状 |
| 4 | `rig_adapter.rs` (rig 框架适配) | 2885 | rig-core 已从 workspace 整个删除（ADR-118） | **死代码** | 不要 port |
| 5 | `anthropic_oauth.rs` (Anthropic OAuth) | 710 | 已被 `AuthSource::BearerToken` + `claw_code_provider` OAuth 优先路径功能性替代 | 已被更优抽象替代 | 不需要 port |
| 6 | `nearai_chat.rs` (NEAR AI Chat) | 2656 | 已在 `desktop-client/ironclaw/src/llm/nearai_chat.rs` | 应用层（依赖 SessionManager） | 维持现状 |
| 7 | `transcription/` (语音转文本) | 593 | 已在 `desktop-client/ironclaw/src/llm/transcription/` | 应用层（channel adapter 耦合） | 维持现状 |

**关键修正**：F3.1 实际只「跳过」了 1 个真协议层缺口（`runtime.rs`），其余 6 个或已按 ADR-118/129 边界正确分层、或已被功能性替代、或本就是死代码。原任务描述中「跳过 7 个」的说法对「跳过」定义偏宽。

## 3. ironclaw-main `SwappableLlmProvider` vs desktop `ModelSwitchProvider` 差异

| 维度 | ironclaw-main runtime.rs | desktop model_switch.rs |
|------|--------------------------|--------------------------|
| 类型名 | `SwappableLlmProvider` | `ModelSwitchProvider` |
| 内部存储 | `RwLock<ProviderSnapshot>`（含 inner + 缓存 metadata） | `Mutex<Arc<dyn LlmProvider>>` |
| 热替换 API | `swap(&self, inner: Arc<dyn LlmProvider>)` | `replace_inner(&self, new: Arc<dyn LlmProvider>)` |
| 并发原语 | `set_model` 与 `swap` 串行（同一 write lock 内 delegate + snapshot 重建） | 单 Mutex |
| metadata 缓存 | 是（cost / model_name / cache_multiplier 原子刷新） | 否（每次现取） |
| 业务字段 | **无**（纯 framework 抽象） | `override_source: Arc<RwLock<Option<String>>>`（per-request model override） |
| 配套类型 | `LlmReloadHandle`（主 + cheap 双 wrapper，并发 reload 串行化） | 无 |
| LOC | 570 | 128 |

**结论**：ironclaw-main 的实现**功能上是 desktop 的超集**，且自带 metadata snapshot 缓存（更适合高频 `complete()` 调用）。desktop 那 128 行是简化版，没有竞争优势。

## 4. F3.1 为何跳过 runtime.rs（事后归因）

`crates/dasclaw_llm_provider/src/lib.rs`、PR #627 / #637 commit message、相关 review comments 都未留下说明。基于代码 + commit 时序证据推断（无明确文档），最可能解释是：

- F3.1 当时的应用层切换已经由 desktop 自带的 `ModelSwitchProvider` 承载（git log 显示该文件早于 F3.1 创建）
- `dasclaw_llm_provider` 被定位为「纯协议层」，于是把 runtime wrapper 判定为「应用层职责」留在调用方实现
- 但这个决策没有写进 `lib.rs` 注释、没有进 ADR-118、没有在 PR description 中说明

这个失误的代价是：

1. desktop 与协议层职责重叠
2. 任何引入 `dasclaw_llm_provider` 的下游（包括未来的无头 agent 框架）若要「运行时切模型」，要么自己再写一份，要么从 desktop 反向依赖（违反 ADR 拆分）

## 5. 决策

### 5.1 协议层（PR #1045）

把 `ironclaw-main/src/llm/runtime.rs` port 到 `crates/dasclaw_llm_provider/src/provider/runtime.rs`，**剥离 ironclaw 应用层依赖**：

- `SessionManager` 注入参数从 `Arc<SessionManager>` 改为闭包：`reload<F, Fut>(build: F) where Fut: Future<Output = Result<ProviderChainComponents, LlmError>>`
- 新增 `pub struct ProviderChainComponents { primary, cheap }` 作为闭包返回值，不依赖任何 ironclaw 类型
- `LlmConfig` 不再传入 reload，由调用方在闭包内自行构造（解耦 dasclaw 与 ironclaw 的 `LlmConfig` 字段差异）
- 模块 `//!` 文档说明：来源（F3.1 遗漏补 port）+ 与 desktop ModelSwitchProvider 的关系 + 设计权衡
- 12 个单元测试覆盖：swap 替换语义、并发 swap 串行、in-flight 调用隔离、reload handle 串行、metadata snapshot 刷新等

### 5.2 应用层（PR #1046）

`desktop-client/src/model_switch.rs` 重构成 `SwappableLlmProvider` 的薄壳：

```rust
pub struct ModelSwitchProvider {
    inner: Arc<SwappableLlmProvider>,                // 委托给协议层
    override_source: Arc<RwLock<Option<String>>>,    // 保留 desktop 业务字段
}
```

所有公共 API 签名不变（`new` / `replace_inner` / `current_inner` / `set_override`），IPC `switch_provider` 命令向后兼容。下游所有调用方（`engine.rs` / `ipc/chat.rs` / `state.rs`）零改动。

### 5.3 顺带修复 `apply_admin_overrides` bug（PR #1046）

`ADMIN_CONFIG_MAPPINGS` 改成结构化清单，每条标注 `requires_companion_key: Option<&str>`。`LLM_BACKEND` 切到不同 backend 时校验对应 companion key：

| backend | companion key |
|---------|---------------|
| `openai` | `OPENAI_API_KEY` |
| `anthropic` | `ANTHROPIC_API_KEY` |
| `openai_compatible` | `LLM_API_KEY` |
| `nearai` | 无（走 session token） |

抽出纯函数 `resolve_overrides(config, current_env) -> Vec<(String, String)>` 做可测形态，5 个场景测试覆盖。后续 `init_default_provider` 通过 `/api/client-models` 拉到完整 `(backend + key + url + model)` 再 `switch_provider` 的路径不变。

### 5.4 不在本 ADR / 本批 PR 范围

- `engine.rs` 启动期改用 `with_llm(handle)` 注入路径（builder 重构） — 留待后续 ADR
- gateway / WebChannel 传 handle 路径 — 同上
- F3.1 跳过的其它 6 个文件 — 经分类确认无需 port（rig/oauth）或已正确分层（session/recording/nearai/transcription）

## 6. 决策反思（流程教训）

本批 PR 的设计过程经历了 3 轮重大修正，每轮都源于「凭印象下断言而未做三层验证」：

| 轮次 | 错误断言 | 真相 | 教训 |
|------|----------|------|------|
| 1 | 「框架缺切换 API，需新增 LlmHandle」 | 框架早有 `set_model` trait method + `FailoverProvider` + `SmartRoutingProvider` | semantic_search 一查就知道 |
| 2 | 「上移 desktop ModelSwitchProvider 到 dasclaw」 | ironclaw-main 早有更成熟的 `SwappableLlmProvider` | 应同时查 ironclaw-main，不只查当前 crate |
| 3 | 「F3.1 跳过 7 文件都该补 port」 | 真正应补的只有 1 个，其它已分层 / 替代 / 死代码 | 「跳过」需要细分语义 |

这与 `user memory: architecture-doc-three-layer-verification.md` 中记录的 doc 53 §4.2 伪缺口教训完全同源。本 ADR 记录此教训以警示后续：

**任何「X 没有 Y」或「X 缺 Y」或「应该新增 Y」的断言，落入 ADR / 14-md / 能力差异表格之前，必须经过三层验证**（semantic_search ≥ 2 种表达 → grep / vscode_listCodeUsages → read_file）。

## 7. 验证

- PR #1045 CI 全绿（Tests / Clippy / Formatting / cargo-deny / 11 个 ADR drift guards / no-panics / blueprint sync / scope）
- PR #1046 CI 全绿（同 PR #1045 + closes-link + regression-test-check）
- 12 个 `SwappableLlmProvider` 单元测试 + 9 个 `ModelSwitchProvider` 行为测试 + 5 个 `resolve_overrides` 场景测试均 PASS
- desktop-client E2E `test_scenario_01_engine_readiness.py` 3/3 PASS（验证引擎在 webdriver 模式正常启动）

## 8. 与现有 ADR 的关系

- 完成 ADR-118 §1.4 三仓 LLM provider 血脉关系最后一块拼图（runtime.rs 归属）
- 验证 ADR-129 verbatim port mandate 的边界：「不能port」也是一种合规决策（rig_adapter / anthropic_oauth）
- 为后续 ADR（无头 agent 框架最小可用形态）扫清了「运行时换 LLM」这个能力缺口

## 9. 关联 PR / Issue

- PR #1045（feat/dasclaw-llm-port-runtime）：协议层 port
- PR #1046（fix/desktop-llm-swappable-handle）：desktop 薄壳重构 + apply_admin_overrides bug 修复
- Issue #1047：PR #1046 跟踪
- PR #1044（fix/tauri-channel-ai-sdk-v5-frames）：本会话首个 PR，修聊天气泡永远 loading（独立 bug，与本 ADR 无关）
