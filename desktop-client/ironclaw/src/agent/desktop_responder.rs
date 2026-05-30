//! Desktop-side `AgentResponder` adapter (ADR-160 §3 L2, W7.3).
//!
//! Houses the LLM call pipeline (cost guard → per-user model override →
//! streaming/non-streaming dispatch → cost record → cache monitor → DB
//! persistence) that previously lived in `ChatDelegate::call_llm`.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use async_trait::async_trait;
use uuid::Uuid;

use dasclaw_core::traits::HostError;
use dasclaw_runtime::AgentResponder;

use crate::channels::{ChannelManager, StatusUpdate};
use crate::error::Error;
use crate::llm::{LlmProvider, Reasoning, ReasoningContext, RespondOutput};
use crate::observability::PromptCacheMonitor;

pub(super) struct DesktopResponder {
    // Invariant: must be constructed per turn. The `model_override_applied`
    // sentinel below is reset on each `new()`; reusing one instance across
    // turns would silently disable per-user `selected_model` overrides.
    reasoning: Reasoning,
    llm: Arc<dyn LlmProvider>,
    llm_backend: String,
    cache_monitor: Option<Arc<PromptCacheMonitor>>,
    tenant: crate::tenant::TenantCtx,
    thread_id: Uuid,
    channels: Arc<ChannelManager>,
    channel: String,
    metadata: serde_json::Value,
    // First-iteration sentinel — replaces the `iteration == 0` check that
    // gated the per-user `selected_model` settings lookup in the previous
    // `ChatDelegate::call_llm` body.
    model_override_applied: AtomicBool,
}

impl DesktopResponder {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn new(
        reasoning: Reasoning,
        llm: Arc<dyn LlmProvider>,
        llm_backend: String,
        cache_monitor: Option<Arc<PromptCacheMonitor>>,
        tenant: crate::tenant::TenantCtx,
        thread_id: Uuid,
        channels: Arc<ChannelManager>,
        channel: String,
        metadata: serde_json::Value,
    ) -> Self {
        Self {
            reasoning,
            llm,
            llm_backend,
            cache_monitor,
            tenant,
            thread_id,
            channels,
            channel,
            metadata,
            model_override_applied: AtomicBool::new(false),
        }
    }

    /// Non-streaming LLM call with context-exceeded retry.
    async fn call_llm_non_streaming(
        &self,
        reason_ctx: &mut ReasoningContext,
    ) -> Result<RespondOutput, Error> {
        let reasoning = &self.reasoning;
        match reasoning.respond_with_tools(reason_ctx).await {
            Ok(output) => Ok(output),
            Err(crate::error::LlmError::ContextLengthExceeded { used, limit }) => {
                tracing::warn!(
                    used,
                    limit,
                    "Context length exceeded, compacting messages and retrying"
                );
                reason_ctx.messages =
                    super::dispatcher::compact_messages_for_retry(&reason_ctx.messages);
                if reason_ctx.force_text {
                    reason_ctx.available_tools.clear();
                }
                reasoning
                    .respond_with_tools(reason_ctx)
                    .await
                    .map_err(|retry_err| {
                        tracing::error!(
                            original_used = used,
                            original_limit = limit,
                            retry_error = %retry_err,
                            "Retry after auto-compaction also failed"
                        );
                        crate::error::Error::from(retry_err)
                    })
            }
            Err(e) => Err(e.into()),
        }
    }

    /// Streaming LLM call: forwards text chunks to the channel while the
    /// response is being generated, with context-exceeded retry fallback.
    async fn call_llm_streaming(
        &self,
        reason_ctx: &mut ReasoningContext,
    ) -> Result<RespondOutput, Error> {
        let reasoning = &self.reasoning;
        let (chunk_tx, mut chunk_rx) = tokio::sync::mpsc::unbounded_channel::<String>();

        let channels = self.channels.clone();
        let channel_name = self.channel.clone();
        let metadata = self.metadata.clone();

        let forward_handle = tokio::spawn(async move {
            while let Some(chunk) = chunk_rx.recv().await {
                let _ = channels
                    .send_status(&channel_name, StatusUpdate::StreamChunk(chunk), &metadata)
                    .await;
            }
        });

        let result = reasoning
            .respond_with_tools_streaming(reason_ctx, chunk_tx)
            .await;

        let _ = forward_handle.await;

        match result {
            Ok(output) => Ok(output),
            Err(crate::error::LlmError::ContextLengthExceeded { used, limit }) => {
                tracing::warn!(
                    used,
                    limit,
                    "Context length exceeded (streaming), compacting and retrying non-streaming"
                );
                reason_ctx.messages =
                    super::dispatcher::compact_messages_for_retry(&reason_ctx.messages);
                if reason_ctx.force_text {
                    reason_ctx.available_tools.clear();
                }
                reasoning
                    .respond_with_tools(reason_ctx)
                    .await
                    .map_err(|retry_err| {
                        tracing::error!(
                            original_used = used,
                            original_limit = limit,
                            retry_error = %retry_err,
                            "Retry after auto-compaction also failed (streaming)"
                        );
                        crate::error::Error::from(retry_err)
                    })
            }
            Err(e) => Err(e.into()),
        }
    }
}

#[async_trait]
impl AgentResponder for DesktopResponder {
    async fn respond(&self, reason_ctx: &mut ReasoningContext) -> Result<RespondOutput, HostError> {
        if let Err(limit) = self.tenant.check_cost_allowed().await {
            return Err(crate::error::LlmError::InvalidResponse {
                provider: "agent".to_string(),
                reason: limit.to_string(),
            }
            .into());
        }

        // Per-user model override from settings. Only applied on the first
        // call within this responder instance — `DesktopResponder` is built
        // once per turn, so this matches the prior `iteration == 0` gate.
        if !self.model_override_applied.swap(true, Ordering::AcqRel)
            && let Some(store) = self.tenant.store()
            && let Ok(Some(value)) = store.get_setting("selected_model").await
            && let Some(model) = value.as_str()
        {
            let model = model.trim();
            if !model.is_empty() {
                reason_ctx.model_override = Some(model.to_string());
            }
        }

        let use_streaming = self.llm.supports_streaming();
        let output = if use_streaming {
            self.call_llm_streaming(reason_ctx).await?
        } else {
            self.call_llm_non_streaming(reason_ctx).await?
        };

        let model_name = self
            .llm
            .effective_model_name(reason_ctx.model_override.as_deref());
        let cost_per_token = if reason_ctx.model_override.is_some() {
            None
        } else {
            Some(self.llm.cost_per_token())
        };
        let read_discount = self.llm.cache_read_discount();
        let write_multiplier = self.llm.cache_write_multiplier();
        let call_cost = self
            .tenant
            .record_llm_call(
                &model_name,
                output.usage.input_tokens,
                output.usage.output_tokens,
                output.usage.cache_read_input_tokens,
                output.usage.cache_creation_input_tokens,
                read_discount,
                write_multiplier,
                cost_per_token,
            )
            .await;
        tracing::debug!(
            "LLM call used {} input + {} output tokens (${:.6})",
            output.usage.input_tokens,
            output.usage.output_tokens,
            call_cost,
        );

        if let Some(ref monitor) = self.cache_monitor {
            monitor.record(
                output.usage.input_tokens,
                output.usage.cache_read_input_tokens,
                output.usage.cache_creation_input_tokens,
                false,
            );
        }

        if let Some(store) = self.tenant.store() {
            let record = crate::history::LlmCallRecord {
                job_id: None,
                conversation_id: Some(self.thread_id),
                provider: &self.llm_backend,
                model: &model_name,
                input_tokens: output.usage.input_tokens,
                output_tokens: output.usage.output_tokens,
                cost: call_cost,
                purpose: Some("chat"),
            };
            if let Err(e) = store.record_llm_call(&record).await {
                tracing::warn!("Failed to persist LLM call to DB: {}", e);
            }
        }

        Ok(output)
    }
}
