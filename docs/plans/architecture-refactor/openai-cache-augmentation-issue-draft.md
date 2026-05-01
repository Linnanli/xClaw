# Issue 草案：feat(llm): OpenAI provider 接入服务端 prompt cache（prompt_cache_key + cached_tokens）

> Status: Draft
> Priority: P1
> 关联 ADR: 不阻塞 [ADR-117](adr-117-p0c-prompt-builder-unification.md)（P0-C prompt builder 统一）；建议在 ADR-117 合并后启动
> Source: P0-C/W3/B 决策讨论（#130）副产品

## 背景 / 问题

当前 desktop（`desktop-client/ironclaw/src/llm/`）的 OpenAI provider 路径在 LLM cache 能力上有两个明确缺口：

### 缺口 1：请求侧未注入 `prompt_cache_key`

`openai_codex_provider.rs` 与 registry openai-protocol 路径都没有给 OpenAI Responses API 发 `prompt_cache_key` 字段（`grep -r prompt_cache_key desktop-client/` 0 处匹配）。
OpenAI 服务端虽然会自动尝试 cache，但若没有 stable cache key，命中率受 hash 抖动影响明显。

参考实现：[`codex-cli-main/codex-rs/core/src/client.rs:878`](../../codex-cli-main/codex-rs/core/src/client.rs#L878)

```rust
prompt_cache_key: Some(self.client.state.conversation_id.to_string()),
```

仅 3 行即生效。

### 缺口 2：响应侧 `cached_tokens` 写死为 0

[`desktop-client/ironclaw/src/llm/openai_codex_provider.rs:270-271, 323-324`](../../desktop-client/ironclaw/src/llm/openai_codex_provider.rs#L270)：

```rust
cache_read_input_tokens: 0,
cache_creation_input_tokens: 0,
```

两处硬编码 — 即使 OpenAI 服务端真的命中了 cache 并在 `usage.prompt_tokens_details.cached_tokens` 里报告了缓存命中数，desktop 完全看不到。后果：

- 计费统计不准（cache 命中实际省了 token 但 metric 不知道）
- `PromptCacheMonitor` 对 OpenAI 路径完全失效（永远看到 0）
- 用户优化 prompt 时无 OpenAI 反馈

## 现状对比矩阵

| Provider | CachedProvider 客户端 | 服务端 prompt_cache_key | cached_tokens 解析 | Monitor 可见 |
|---|---|---|---|---|
| Anthropic | ✅ | ✅ ephemeral marker | ✅ | ✅ |
| OpenAI codex | ✅ | ❌ | ❌ 写死 0 | ❌ |
| OpenAI compat (registry) | ✅ | ❌ | ❌ | ❌ |

## 范围

**P1，不阻塞**。与 P0-C / [ADR-117](adr-117-p0c-prompt-builder-unification.md) prompt builder 统一**完全正交**，建议在 ADR-117 合并后启动。

## 改动清单

### 1. 注入 `prompt_cache_key`

- `desktop-client/ironclaw/src/llm/openai_codex_provider.rs`：构造 OpenAI Responses API request 时填 `prompt_cache_key = session_id.to_string()`
- registry openai-protocol provider：同步注入
- 决策点：用 session_id 还是 conversation_id？建议 session_id（与 codex 等价但避免 desktop 已有 session/conversation 二级概念冲突）

### 2. 解析响应 `cached_tokens`

- `openai_codex_provider.rs:270/271/323/324`：从 OpenAI Responses API 响应的 `usage.prompt_tokens_details.cached_tokens` 读取
- 映射规则：
  - `cache_read_input_tokens` ← `usage.prompt_tokens_details.cached_tokens`
  - `cache_creation_input_tokens` 保持 0（OpenAI 无 ephemeral 写入概念，符合协议本质）

### 3. PromptCacheMonitor 通用化

- `desktop-client/ironclaw/src/llm/observability/prompt_cache.rs`：保持现有 record 接口；OpenAI 路径填 `cache_creation_input_tokens=0` 即可（最小改动方案）
- 不需要新增 `provider_kind` 字段（避免 API 变更）

### 4. 测试

- mock-openai-service 路径增加 cache hit 测试（mock 服务返回 `usage.prompt_tokens_details.cached_tokens > 0`）
- 新增集成测试 `req_llm_openai_cache_001`：连续两次相同 prompt → 第二次 `cache_read_input_tokens > 0`
- 单测：`prompt_cache_key` 正确从 session_id 派生

## 不做（避免范围蔓延）

- 不动 LayeredPromptBuilder（属 [ADR-117](adr-117-p0c-prompt-builder-unification.md) / P0-C）
- 不引入 OpenAI Responses API 全套迁移（仅 cache 字段）
- 不改 CacheRetention enum（OpenAI 无 TTL 客户端控制）
- 不增加 CachedProvider TTL 跟随 provider（保持 1h 默认）

## 验证

```bash
cargo nextest run -p ironclaw llm::openai
cargo nextest run -p ironclaw llm::observability::prompt_cache
```

集成验证：连续相同 prompt 两次 → `tracing::info!` 日志可见 `cache_read_input_tokens > 0`。

## 收益估算

- OpenAI 用户场景下 token 计费可见性恢复
- `PromptCacheMonitor` 命中率指标对 OpenAI 路径可用
- 长会话场景理论 token 节省（依赖 OpenAI 服务端策略，预计 30-60% input token 减少）

## 复制 codex / 自研比例

- 复制部分：`prompt_cache_key` 注入逻辑（约 3 行）+ `cached_tokens` 字段读取（约 5 行）
- 自研部分：在 desktop 多 provider trait + `PromptCacheMonitor` 观测体系下集成（约 100-200 行）
- desktop 比 codex 更好的部分：多 provider 抽象 + `CachedProvider`（response_cache.rs）客户端响应缓存 + Monitor 可观测性

## 来源参考

- codex 实现：[`codex-cli-main/codex-rs/core/src/client.rs:878`](../../codex-cli-main/codex-rs/core/src/client.rs#L878)（prompt_cache_key 注入）
- desktop 现状：[`desktop-client/ironclaw/src/llm/openai_codex_provider.rs:270-271, 323-324`](../../desktop-client/ironclaw/src/llm/openai_codex_provider.rs#L270)
- 客户端缓存层（已有，无需改动）：[`desktop-client/ironclaw/src/llm/response_cache.rs`](../../desktop-client/ironclaw/src/llm/response_cache.rs)
- Monitor：[`desktop-client/ironclaw/src/llm/observability/prompt_cache.rs`](../../desktop-client/ironclaw/src/llm/observability/prompt_cache.rs)
