# P0 W6-C ironclaw 调用面 inventory（PR-A.4 准备）

> **状态**：inventory（只读盘点）。本文件**不做架构决策**，只列事实。
>
> **目的**：在 PR-A.4 切换 `desktop-client/ironclaw` 的 `claw-code-api` path-dep 到
> `dasclaw_llm_provider` 之前，把所有 ironclaw 端依赖 `claw_code_api::*` 的位置、签名、
> 已移植/未移植状态全列出来，PR-A.4 实施时按本表逐条迁移。
>
> **来源**：
> - Level 1 `semantic_search`：claude code api provider usage send_message stream
> - Level 2 引用面：`grep_search "claw_code_api"` in `desktop-client/**/*.rs`（11 hit）+ Cargo.toml（5 hit）
> - Level 3 字面量：`grep` `pub fn (with_|new|from_auth|complete|stream)` 双 client 比对
>
> **关联 ADR**：[ADR-118 §5 W6-C](adr-118-claw-code-readonly-and-self-impl.md#5-w6-c) /
> [§8 实施事实账本](adr-118-claw-code-readonly-and-self-impl.md#8)

## 1. 调用面总图

ironclaw 中**唯一**消费 `claw-code-api` 的文件是 [`desktop-client/ironclaw/src/llm/claw_code_provider.rs`](../../desktop-client/ironclaw/src/llm/claw_code_provider.rs)（1351 LOC）。
Cargo path-dep 在 [`desktop-client/ironclaw/Cargo.toml`](../../desktop-client/ironclaw/Cargo.toml#L149)：

```toml
claw-code-api = { path = "../../claw-code/rust/crates/api", package = "api" }
```

无第二个消费方。

## 2. 引用符号清单（共 16 个）

来源：`claw_code_provider.rs` 第 13-17 行 `use claw_code_api::{...}` 块 + 散落 `claw_code_api::xxx` 调用 + `tests` 块的 `use claw_code_api::Usage` / `use claw_code_api::ProviderKind`。

| # | 符号 | 引用方式 | 在 `dasclaw_llm_provider` 已移植？ | 备注 |
|---|---|---|---|---|
| 1 | `AnthropicClient` | type | ✅ PR-A.2 | 字段名/构造方法有差异（见 §3.1） |
| 2 | `AuthSource` | enum | ✅ PR-A.2 | 三变体齐全（`ApiKey` / `BearerToken` / `ApiKeyAndBearer`） |
| 3 | `InputContentBlock` | type | ✅ PR-A.0 types | |
| 4 | `InputMessage` | type | ✅ PR-A.0 types | `role: String`，非 enum |
| 5 | `MessageRequest` | type | ✅ PR-A.0 types | |
| 6 | `MessageResponse` | type | ✅ PR-A.0 types | |
| 7 | `OpenAiCompatClient` | type | ✅ PR-A.3 | `complete()` only，`stream()` 留 PR-A.3.1 |
| 8 | `OpenAiCompatConfig` | type | ✅ PR-A.3 | `openai()` / `xai()` / `dashscope()` 三预设齐 |
| 9 | `OutputContentBlock` | type | ✅ PR-A.0 types | |
| 10 | `ProviderClient` | enum + dispatcher | ❌ **未移植** | **PR-A.4 必须新增**，见 §3.2 |
| 11 | `ToolChoice` | enum | ✅ PR-A.0 types | ironclaw 重命名为 `ApiToolChoice` |
| 12 | `ToolDefinition` | struct | ✅ PR-A.0 types | ironclaw 重命名为 `ApiToolDefinition` |
| 13 | `ToolResultContentBlock` | struct | ✅ PR-A.0 types | |
| 14 | `ApiError` | error type | ✅ PR-A.0 error | dasclaw 拆变体更细 |
| 15 | `Usage` | struct | ✅ PR-A.0 types | 测试块用 |
| 16 | `ProviderKind` | enum | ✅ PR-A.0 providers | 测试块用 |

## 3. 函数 / 方法调用清单

### 3.1 已移植但**签名不兼容**（PR-A.4 必须改 ironclaw 调用方）

| 调用位置（claw_code_provider.rs 行号） | 当前调用 | 目标 dasclaw 等价 | 签名差异 |
|---|---|---|---|
| L45, L77, L415 | `claw_code_api::resolve_model_alias(model) -> &'static str` | `dasclaw_llm_provider::resolve_model_alias(model) -> String` | 返回类型 `&'static str` → `String`，调用方已 `.to_string()` 兼容 |
| L416 | `claw_code_api::detect_provider_kind(&resolved)` | `dasclaw_llm_provider::detect_provider_kind(&resolved, &env)` | **新增 `EnvSnapshot` 参数**：调用方需先 `EnvSnapshot::from_process_env()` 然后传引用 |
| L411 (`build_anthropic_client`) | `AnthropicClient::from_auth(auth)` 返回 `Self` | `AnthropicClient::with_auth(auth)?` 返回 `Result<Self, ApiError>` | **可错构造**：调用方需 `?` 传播错误并 `map_err(map_api_error)` |
| L449 (`build_openai_compat_client`) | `OpenAiCompatClient::new(key, config)` 返回 `Self` | `OpenAiCompatClient::new(key, config)?` 返回 `Result<Self, ApiError>` | **可错构造**：同上 |
| L506, L539（`complete` / `complete_with_tools` 内 `self.client.send_message(&msg_req)`） | 通过 `ProviderClient` enum dispatch 到具体 client 的 `send_message` | 见 §3.2 — `ProviderClient` 还没移植 | **方法名也需考虑**：dasclaw 把 client 单点方法叫 `complete()`，不是 `send_message()` |

### 3.2 未移植：`ProviderClient` enum dispatcher（PR-A.4 必须新增）

claw-code 的 `ProviderClient` 提供三件事，ironclaw 全部依赖：

```rust
// claw-code/rust/crates/api/src/client.rs:10-104
pub enum ProviderClient {
    Anthropic(AnthropicClient),
    Xai(OpenAiCompatClient),
    OpenAi(OpenAiCompatClient),
}
impl ProviderClient {
    pub fn from_model(model: &str) -> Result<Self, ApiError> { /* 别名 → kind → 默认 client */ }
    pub fn from_model_with_anthropic_auth(model, anthropic_auth: Option<AuthSource>) -> Result<Self, ApiError>;
    pub const fn provider_kind(&self) -> ProviderKind;
    pub async fn send_message(&self, req: &MessageRequest) -> Result<MessageResponse, ApiError>;
    pub async fn stream_message(&self, req: &MessageRequest) -> Result<MessageStream, ApiError>;
    // 还有 prompt_cache 系列，ironclaw 未消费 → PR-A.4 不必移植
}
```

ironclaw 实际消费点（`claw_code_provider.rs`）：
- L46：`ProviderClient::from_model(&configured_model).map_err(map_api_error)?`
- L412：`ProviderClient::Anthropic(client)` 构造（`build_anthropic_client` 末尾）
- L455-458：`ProviderClient::Xai(client)` / `ProviderClient::OpenAi(client)` 构造（`build_openai_compat_client`）
- L464：`ProviderClient::OpenAi(client)`（`build_ollama_client`）
- L506, L539：`self.client.send_message(&msg_req).await`

**ironclaw 未消费**的：`stream_message` / `provider_kind` / `prompt_cache*` / `from_model_with_anthropic_auth`。

### 3.3 已移植且签名兼容（PR-A.4 改 import path 即可）

| 符号 | claw-code 路径 | dasclaw 路径 |
|---|---|---|
| `InputContentBlock` / `InputMessage` / `MessageRequest` / `MessageResponse` / `OutputContentBlock` / `ToolChoice` / `ToolDefinition` / `ToolResultContentBlock` / `Usage` | `claw_code_api::*` | `dasclaw_llm_provider::*`（已 re-export） |
| `ApiError` | `claw_code_api::ApiError` | `dasclaw_llm_provider::ApiError` |
| `ProviderKind` | `claw_code_api::ProviderKind` | `dasclaw_llm_provider::ProviderKind` |
| `AuthSource` | `claw_code_api::AuthSource` | `dasclaw_llm_provider::AuthSource` |
| `AnthropicClient::with_base_url` / `OpenAiCompatClient::with_base_url` | 同名 | 同名（builder pattern 一致） |

## 4. PR-A.4 必做改动清单（按 file 分块）

### 4.1 `desktop-client/ironclaw/Cargo.toml`

- 删除 `claw-code-api = { path = "../../claw-code/rust/crates/api", package = "api" }`（L149）
- 新增 `dasclaw_llm_provider = { path = "../../crates/dasclaw_llm_provider" }`
- 注释（L147-148, L231-232）一并更新或删除

### 4.2 `desktop-client/ironclaw/src/llm/claw_code_provider.rs`

按 §3.1 + §3.2 改：

1. `use` 块全部改 `claw_code_api::` → `dasclaw_llm_provider::`
2. `from_model` (L46) 调用方：先 `let env = EnvSnapshot::from_process_env();` 再两步调（kind 检测 + 构造），或者**在 dasclaw 增 `ProviderClient::from_model(model)` 兼容方法**
3. `AnthropicClient::from_auth(auth)` (L411) → `with_auth(auth)?`
4. `OpenAiCompatClient::new(...)` (L449, L460) 加 `?`
5. `detect_provider_kind(&resolved)` (L416) 加 `EnvSnapshot` 参数
6. `client.send_message(&msg_req)` (L506, L539) 改 `client.complete(&msg_req)` 或在 dasclaw `ProviderClient` 上提供 `send_message` alias
7. 测试块 (L534, L1006) `use` 改路径

### 4.3 选项 A vs 选项 B（PR-A.4 设计决策点，不在本 inventory 决断）

- **选项 A — ironclaw 全面适配 dasclaw 新签名**：调用方写 `EnvSnapshot::from_process_env()`、`with_auth(...)?`、`.complete(...)`，dasclaw 不加任何兼容层。**优点**：dasclaw API 干净；**缺点**：PR-A.4 diff 大。
- **选项 B — dasclaw 加薄兼容层**：在 dasclaw 提供 `ProviderClient::from_model(&str)`（内部读 env）、`send_message` 作为 `complete` 的 alias、`AnthropicClient::from_auth(auth) -> Self` 内部 unwrap_or_panic。**优点**：PR-A.4 改动小；**缺点**：dasclaw 沾上"无 env 副作用"原则的反例（ADR-118 §8.5 第 2 点明确说 dasclaw 不读 env）。

**推荐**：**选项 A**。ADR-118 §8.5 已经把"无 env 副作用"列为顺势处理的架构债，PR-A.4 一次性还清，比留兼容层后续再拆更彻底。`ProviderClient` enum 仍然在 dasclaw 提供（因 ironclaw 真的需要 enum dispatch），但 `from_model` 改成 `from_model(model, &env)` 显式签名。

## 5. 已确认的非目标（PR-A.4 不做）

1. **`OpenAiCompatClient::stream()`** 移植 — 留 PR-A.3.1，ironclaw 当前 call site 全部 `send_message` (`stream: false`)
2. **`AnthropicClient::stream()` 在 `ProviderClient::stream_message` 上的 dispatch** — ironclaw 未消费，dasclaw `ProviderClient::stream_message` 不必先实现（PR-A.4 可只暴露 `send_message` dispatch）
3. **`prompt_cache*` 方法族** — ironclaw 未消费
4. **`from_env*` 工厂方法**（`AnthropicClient::from_env`、`OpenAiCompatClient::from_env`、`AnthropicClient::from_env_or_saved`）— ironclaw `build_anthropic_client` / `build_openai_compat_client` 都从 `RegistryProviderConfig` 显式构造 `AuthSource`，不走 env，无须移植
5. **`ProviderClient::with_prompt_cache` / `prompt_cache_stats` / `take_last_prompt_cache_record`** — ironclaw 未消费
6. **`ProviderClient::from_model_with_anthropic_auth`** — ironclaw 未消费（OAuth 走 `build_anthropic_client` 显式）
7. **删除 `claw-code` 子仓本身** — ADR-118 §5 已声明 W6-C 后子仓只读保留，不删除

## 6. 验证清单（PR-A.4 本地最低门）

按 [AGENTS.md "本地快速开发节奏"](../../AGENTS.md) **档 2 推荐**（基础库改下游消费方）：

```bash
# 1. 本 crate
cargo check -p dasclaw_llm_provider --tests
cargo nextest run -p dasclaw_llm_provider

# 2. 下游 — 基础库改动必跑
cargo check --workspace --tests          # ironclaw + desktop-client + admin-backend 全编

# 3. 局部测试
cargo nextest run -p ironclaw --lib llm
cargo build -p desktop-client --lib --tests   # link-time / Tauri state 验证

# 4. 标准三件套
cargo fmt --all
python3.12 scripts/check_no_panics.py --base origin/xClaw
cargo clippy --no-deps -p dasclaw_llm_provider --all-targets -- -D warnings
cargo clippy --no-deps -p ironclaw --all-targets -- -D warnings
```

## 7. 三层验证证据存档

按 [AGENTS.md 分析工具使用规范](../../AGENTS.md) 要求，本 inventory 是「跨项目对账 + 否定性结论」，已完成三层验证：

- **Level 1 `semantic_search`**：query "ironclaw claude code api provider usage send_message stream" → 命中 `claw_code_provider.rs` / `claw_code_api::*` import 块 / `ProviderClient` 实现 / dasclaw_llm_provider 当前 re-export 状态。
- **Level 2 `grep_search`**：`claw_code_api` in `desktop-client/**/*.rs` → 11 hit（全部在 `claw_code_provider.rs`）；`claw-code-api|claw_code_api` in `desktop-client/**/Cargo.toml` → 5 hit（仅 ironclaw/Cargo.toml）。
- **Level 3 `grep_search`**：`pub fn (with_|new|from_auth|complete|stream|base_url)` 在 dasclaw 双 client 与 claw-code 双 client 上分别枚举 → 签名差异表（§3.1）。

**否定性结论 + 双证据**：
- "ironclaw 未消费 stream_message" — Level 2 grep `stream_message|stream(` in `desktop-client/ironclaw/src/llm/*.rs` 无 hit + Level 1 ADR-118 §8 事实账本表"streaming = ❌（trait 默认）"
- "ironclaw 未消费 prompt_cache*" — Level 2 grep `prompt_cache` in `desktop-client/ironclaw/src/llm/*.rs` 无 hit
- "`ProviderClient` 在 dasclaw 不存在" — Level 2 grep `ProviderClient` in `crates/dasclaw_llm_provider/src/**` 无 hit + Level 1 lib.rs re-export 列表无该名

---

**文档版本**：v1（inventory 初稿，伴随 PR-A.4 起草）
**生成时间**：随 PR-A.4 准备工作创建
**生成方法**：`semantic_search` → `grep_search` → `read_file` 三层验证后人工汇总
