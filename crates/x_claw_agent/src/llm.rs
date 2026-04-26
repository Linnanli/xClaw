//! `x_claw_agent::llm` — Facade 合同：未来 `dasclaw_core::llm::claw_code` 的接口边界。
//!
//! ## 背景与定位
//!
//! `desktop-client/ironclaw/src/llm/claw_code_provider.rs`（1,334 LOC）是
//! Claude Code 协议适配的核心入口，目前住在 fork 内，依赖 fork 私有的：
//!
//! - `crate::llm::provider::LlmProvider` trait（141 LOC，11 个 method）
//! - `crate::llm::error::LlmError` 错误类型
//! - `crate::llm::config::RegistryProviderConfig` 配置结构
//! - `crate::llm::registry::ProviderProtocol` 枚举
//!
//! 详见 [`docs/plans/architecture-refactor/39-fork-private-cargo-inventory.md`]
//! §1.1，预计 W6 删 fork 时整段（≈ 5,000~8,000 LOC，整个 `src/llm/` 目录）
//! 搬到 `dasclaw_core::llm`。
//!
//! ## 为什么先立 facade 合同（C-1 阶段）
//!
//! 直接搬 1,334 LOC 风险点：
//! 1. fork 现有 `LlmProvider` trait 11 个 method 不一定全部都该上 dasclaw_core；
//!    cost / cache / streaming 等可能要重塑或下沉到子 trait。
//! 2. 错误类型 `LlmError` 强耦合 fork 应用层（含 `WorkspaceError` 路径），
//!    搬过来必须中性化为 `Box<dyn Error>` 或专用错误。
//! 3. 不在 dasclaw_core 这一边把"未来必须满足的最小窄接口"先固化，搬迁时
//!    每改一处都要往复重画形状。
//!
//! 本模块**只定义合同 trait，不实现，不引入新依赖**。fork 内现有实现
//! 暂不被强制继承（避免 W2 之外的侵入改动），等 W6 整段搬到
//! dasclaw_core 时一并 `impl LlmProviderFacade for ClawCodeLlmProvider`。
//!
//! ## 与 [`crate::traits::LlmCompleter`] 的关系
//!
//! `LlmCompleter` 是更窄的 facade，只支持「单次 completion 取干净文本」
//! 用例（compaction / heartbeat / slash-command），错误用 `HostError`
//! boxed dyn。`LlmProviderFacade` 是更完整的 provider 接口，覆盖工具调用、
//! 流式响应、模型切换 — 但仍然故意不暴露 cost/cache 计算（这些保留在
//! ironclaw fork，等 dasclaw_core 决定是否上 facade 时再迁）。
//!
//! 一个具体 provider 通常 `impl LlmProviderFacade` + `impl LlmCompleter`
//! 两者，前者给 agent loop 主路径，后者给辅助 facade。
//!
//! ## 合同决策表（W6 搬迁时按此表落地）
//!
//! | fork 现有 method | facade keep? | 理由 |
//! |------------------|--------------|------|
//! | `model_name() -> &str` | ✅ keep | 标识，零代价 |
//! | `complete(req) -> Resp` | ⚠️ drop | 已被 `complete_with_tools` 覆盖；保留多一份 trait method 没必要 |
//! | `complete_with_tools(req) -> Resp` | ✅ keep（核心） | agent loop 主入口 |
//! | `complete_with_tools_stream(req, tx)` | ✅ keep | UI streaming 必须 |
//! | `list_models() -> Vec<String>` | ✅ keep | provider 配置/注册 UI 用 |
//! | `model_metadata() -> ModelMetadata` | ✅ keep | context length 决策用 |
//! | `effective_model_name(req)` | ✅ keep | 多模型 alias 路由 |
//! | `active_model_name()` | ⚠️ 合并 | 与 `effective_model_name(None)` 等价，drop |
//! | `set_model(model)` | ✅ keep | 运行时切模型 |
//! | `cost_per_token() -> (Decimal, Decimal)` | ❌ drop | 留在 ironclaw 应用层，不上 dasclaw_core |
//! | `calculate_cost(in, out) -> Decimal` | ❌ drop | 同上 |
//! | `cache_write_multiplier()` | ❌ drop | 同上 |
//! | `cache_read_discount()` | ❌ drop | 同上 |
//! | `supports_streaming() -> bool` | ✅ keep | 调用方判定路径用 |
//!
//! 即：**11 个 method 中 7 个进 facade，3 个 cost 相关留在 fork/ironclaw
//! 应用层，1 个合并**。
//!
//! ## 错误类型策略
//!
//! `LlmProviderFacade` 用 [`HostError`](crate::traits::HostError)（即
//! `Box<dyn std::error::Error + Send + Sync>`），与 `LlmCompleter` 一致。
//! 具体 provider 实现仍可在内部用 fork 的 `LlmError`，调用方只看到 boxed
//! dyn。这样错误细节既能保留（downcast 取回），又不污染 dasclaw_core。

use async_trait::async_trait;

use crate::messages::{
    CompletionRequest, CompletionResponse, ModelMetadata, ToolCompletionRequest,
    ToolCompletionResponse,
};
use crate::traits::HostError;

/// LLM Provider 公共合同（W6 搬迁后 `dasclaw_core::llm::LlmProvider` 的形状）。
///
/// 本 trait 当前**没有任何实现**，仅作合同声明。fork 内
/// `desktop-client/ironclaw/src/llm/provider.rs` 的 `LlmProvider` trait 暂
/// 不继承本 trait（避免 W2 之外的侵入改动）。W6 搬到 dasclaw_core 时，会
/// (a) 把现有 11 个 method 按合同表收敛到本 trait，(b) cost / cache 相关
/// 保留在 ironclaw 应用层的扩展 trait 上。
///
/// 实现者必须 `Send + Sync`（agent loop 多线程调度）。
#[async_trait]
pub trait LlmProviderFacade: Send + Sync {
    /// 模型标识（注册时配置的名字，可能是 alias）。
    fn model_name(&self) -> &str;

    /// 当前是否支持流式响应。返回 `false` 时
    /// [`complete_with_tools_stream`](Self::complete_with_tools_stream)
    /// 默认 fallback 到非流式路径。
    fn supports_streaming(&self) -> bool {
        false
    }

    /// 解析请求里 `model` 字段的最终值（处理 alias / 默认值）。
    fn effective_model_name(&self, requested_model: Option<&str>) -> String;

    /// 运行时切模型（不是所有 provider 都支持）。
    fn set_model(&self, model: &str) -> Result<(), HostError>;

    /// 列出 provider 当前可用的所有模型 ID。
    async fn list_models(&self) -> Result<Vec<String>, HostError> {
        Ok(Vec::new())
    }

    /// 取当前模型的元信息（context length 等）。默认返回模型名 + 无 size。
    async fn model_metadata(&self) -> Result<ModelMetadata, HostError> {
        Ok(ModelMetadata {
            id: self.model_name().to_string(),
            context_length: None,
        })
    }

    /// 单次 completion（无工具）。注意：W6 搬迁后可能会被
    /// [`complete_with_tools`](Self::complete_with_tools) 完全覆盖并 drop。
    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse, HostError>;

    /// 带工具的 completion，agent loop 主入口。
    async fn complete_with_tools(
        &self,
        request: ToolCompletionRequest,
    ) -> Result<ToolCompletionResponse, HostError>;

    /// 流式带工具 completion。`chunk_tx` 接收增量文本片段，最终返回完整
    /// 响应（含全部工具调用）。
    ///
    /// 默认实现 fallback 到非流式 + 整段一次性发送，保证不支持流式的
    /// provider 不必单独实现。
    async fn complete_with_tools_stream(
        &self,
        request: ToolCompletionRequest,
        chunk_tx: tokio::sync::mpsc::UnboundedSender<String>,
    ) -> Result<ToolCompletionResponse, HostError> {
        let response = self.complete_with_tools(request).await?;
        if let Some(ref content) = response.content {
            let _ = chunk_tx.send(content.clone());
        }
        Ok(response)
    }
}

#[cfg(test)]
mod tests {
    //! Compile-time only smoke：确保 trait 可以被 dyn-dispatch（agent loop
    //! 持有 `Arc<dyn LlmProviderFacade>` 是核心使用形态）。

    use super::*;

    fn _assert_object_safe(_: &dyn LlmProviderFacade) {}

    #[test]
    fn trait_is_object_safe() {
        // 编译过即代表 LlmProviderFacade 是 object-safe 的（agent loop
        // 主路径会持有 Arc<dyn LlmProviderFacade>）。
        // 这里没有具体实现可塞，所以仅做 compile-time 验证。
        fn _check<T: LlmProviderFacade + ?Sized>() {}
        _check::<dyn LlmProviderFacade>();
    }
}
