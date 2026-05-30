//! Desktop-side `AgentResponder` adapter (ADR-160 §3 L2, W7.3/W7.4).
//!
//! Houses the LLM call pipeline (cost guard → per-user model override →
//! pre-LLM iteration setup → streaming/non-streaming dispatch → cost record →
//! cache monitor → DB persistence) that previously lived in
//! `ChatDelegate::call_llm` + `ChatDelegate::before_llm_call`.

use std::collections::HashSet;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use async_trait::async_trait;
use uuid::Uuid;

use dasclaw_core::messages::ChatMessage;
use dasclaw_core::traits::HostError;
use dasclaw_runtime::AgentResponder;

use crate::channels::{ChannelManager, StatusUpdate};
use crate::error::Error;
use crate::llm::{LlmProvider, Reasoning, ReasoningContext, RespondOutput};
use crate::observability::PromptCacheMonitor;

pub(super) struct DesktopResponder {
    // Invariant: must be constructed per turn. The `model_override_applied`
    // and `iteration` sentinels below are reset on each `new()`; reusing
    // one instance across turns would silently disable per-user
    // `selected_model` overrides and skew the iteration-driven nudge /
    // force-text gates.
    reasoning: Reasoning,
    llm: Arc<dyn LlmProvider>,
    llm_backend: String,
    cache_monitor: Option<Arc<PromptCacheMonitor>>,
    tenant: crate::tenant::TenantCtx,
    thread_id: Uuid,
    channels: Arc<ChannelManager>,
    channel: String,
    metadata: serde_json::Value,
    // W7.4: tool-table refresh / nudge / force-text inputs (moved from
    // ChatDelegate::before_llm_call).
    tools: Arc<crate::tools::ToolRegistry>,
    active_skills: Vec<crate::skills::LoadedSkill>,
    disabled_extensions: HashSet<String>,
    cached_prompt: String,
    cached_prompt_no_tools: String,
    nudge_at: usize,
    force_text_at: usize,
    // Per-turn iteration counter; replaces the `iteration` parameter the
    // old `LoopDelegate::before_llm_call` received but `AgentResponder`
    // does not expose.
    iteration: AtomicUsize,
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
        tools: Arc<crate::tools::ToolRegistry>,
        active_skills: Vec<crate::skills::LoadedSkill>,
        disabled_extensions: HashSet<String>,
        cached_prompt: String,
        cached_prompt_no_tools: String,
        nudge_at: usize,
        force_text_at: usize,
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
            tools,
            active_skills,
            disabled_extensions,
            cached_prompt,
            cached_prompt_no_tools,
            nudge_at,
            force_text_at,
            iteration: AtomicUsize::new(0),
            model_override_applied: AtomicBool::new(false),
        }
    }

    /// Pre-LLM iteration setup: tool table refresh, nudge injection,
    /// force-text gating, prompt swap, status broadcast. Moved verbatim
    /// from the deleted `ChatDelegate::before_llm_call`.
    async fn before_llm_call(&self, reason_ctx: &mut ReasoningContext, iteration: usize) {
        if iteration == self.nudge_at {
            reason_ctx.messages.push(ChatMessage::system(
                "You are approaching the tool call limit. \
                 Provide your best final answer on the next response \
                 using the information you have gathered so far. \
                 Do not call any more tools.",
            ));
        }

        let force_text = iteration >= self.force_text_at;

        let tool_defs = self
            .tools
            .tool_definitions_for_llm(
                &dasclaw_governance::tool_visibility::ToolGateContextSeed::system(
                    dasclaw_governance::tool_visibility::Env::Interactive,
                ),
            )
            .await;
        let tool_defs = super::dispatcher::filter_tools_by_disabled_extensions(
            tool_defs,
            &self.disabled_extensions,
        );
        let tool_defs = if !self.active_skills.is_empty() {
            let result = crate::skills::attenuate_tools(&tool_defs, &self.active_skills);
            tracing::debug!(
                min_trust = %result.min_trust,
                tools_available = result.tools.len(),
                tools_removed = result.removed_tools.len(),
                removed = ?result.removed_tools,
                explanation = %result.explanation,
                "Tool attenuation applied"
            );
            result.tools
        } else {
            tool_defs
        };

        reason_ctx.available_tools = tool_defs;
        let force_text = force_text || reason_ctx.force_text;
        reason_ctx.system_prompt = Some(if force_text {
            self.cached_prompt_no_tools.clone()
        } else {
            self.cached_prompt.clone()
        });
        reason_ctx.force_text = force_text;

        if force_text {
            tracing::info!(
                iteration,
                "Forcing text-only response (iteration limit reached)"
            );
        }

        let _ = self
            .channels
            .send_status(
                &self.channel,
                StatusUpdate::Thinking(format!("Thinking (step {iteration})...")),
                &self.metadata,
            )
            .await;
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
        let iteration = self.iteration.fetch_add(1, Ordering::Relaxed);
        self.before_llm_call(reason_ctx, iteration).await;

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
