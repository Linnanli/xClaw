//! W7.1 byte-equivalent extraction of `ChatDelegate::execute_tool_calls`
//! (ADR-160 §3 L2 desktop dispatcher implementation).
//!
//! W7.2 will let this struct `impl dasclaw_runtime::ToolDispatcher`;
//! W7.3 will wire it through `AgenticLoop`. This slice keeps behaviour
//! unchanged — the original method now delegates to `dispatch`.

use std::collections::HashSet;
use std::sync::Arc;

use tokio::sync::Mutex;
use tokio::task::JoinSet;
use uuid::Uuid;

use crate::agent::Agent;
use crate::agent::agentic_loop::LoopOutcome;
use crate::agent::session::{PendingApproval, Session};
use crate::channels::{IncomingMessage, StatusUpdate};
use crate::error::Error;
use crate::llm::{ChatMessage, ReasoningContext};
use crate::tools::redact_params;
use dasclaw_core::traits::HostError;
use dasclaw_runtime::context::JobContext;

use super::dispatcher::{
    PreflightOutcome, check_auth_required, contextual_tool_message, execute_chat_tool_standalone,
    is_tool_disabled_by_extensions, parse_auth_result, preflight_rejection_tool_message,
};

pub(super) struct DesktopDispatcher<'a> {
    pub(super) agent: &'a Agent,
    pub(super) message: &'a IncomingMessage,
    pub(super) session: &'a Arc<Mutex<Session>>,
    pub(super) thread_id: Uuid,
    pub(super) job_ctx: &'a JobContext,
    pub(super) disabled_extensions: &'a HashSet<String>,
    pub(super) user_tz: chrono_tz::Tz,
}

impl<'a> DesktopDispatcher<'a> {
    pub(super) async fn dispatch(
        &self,
        tool_calls: Vec<crate::llm::ToolCall>,
        content: Option<String>,
        usage: dasclaw_core::TokenUsage,
        reason_ctx: &mut ReasoningContext,
    ) -> Result<Option<LoopOutcome>, HostError> {
        // Extract and sanitize the narrative before consuming `content`.
        // ADR-148 R2: route through the unified Layer-B egress gate
        // (`UserDisplay` kind) via `safety::egress::sanitize_tool_output_via_egress`
        // instead of the legacy `SafetyLayer::sanitize_tool_output` path.
        let narrative = match content.as_deref().filter(|c| !c.trim().is_empty()) {
            Some(c) => {
                let sanitized = crate::safety::egress::sanitize_tool_output_via_egress(
                    self.agent.safety(),
                    "agent_narrative",
                    c,
                )
                .await;
                if sanitized.trim().is_empty() {
                    None
                } else {
                    Some(sanitized)
                }
            }
            None => None,
        };

        // Add the assistant message with tool_calls to context.
        // OpenAI protocol requires this before tool-result messages.
        reason_ctx.messages.push(
            ChatMessage::assistant_with_tool_calls(content, tool_calls.clone()).with_usage(usage),
        );

        // Execute tools and add results to context
        let _ = self
            .agent
            .channels
            .send_status(
                &self.message.channel,
                StatusUpdate::Thinking(contextual_tool_message(&tool_calls)),
                &self.message.metadata,
            )
            .await;

        // Build per-tool decisions for the reasoning update.
        // ADR-148 R2: route every tool rationale through the unified
        // Layer-B egress gate (`UserDisplay` kind). Build sequentially
        // (`for`) instead of `.filter_map` so each gate call can `.await`.
        let mut decisions: Vec<crate::channels::ToolDecision> =
            Vec::with_capacity(tool_calls.len());
        for tc in tool_calls.iter() {
            if let Some(r) = tc.reasoning.as_ref() {
                let sanitized = crate::safety::egress::sanitize_tool_output_via_egress(
                    self.agent.safety(),
                    "tool_rationale",
                    r,
                )
                .await;
                decisions.push(crate::channels::ToolDecision {
                    tool_name: tc.name.clone(),
                    rationale: sanitized,
                });
            }
        }

        // Emit reasoning update to channels.
        if narrative.is_some() || !decisions.is_empty() {
            let _ = self
                .agent
                .channels
                .send_status(
                    &self.message.channel,
                    StatusUpdate::ReasoningUpdate {
                        narrative: narrative.clone().unwrap_or_default(),
                        decisions: decisions.clone(),
                    },
                    &self.message.metadata,
                )
                .await;
        }

        // Record tool calls in the thread with sensitive params redacted.
        {
            let mut redacted_args: Vec<serde_json::Value> = Vec::with_capacity(tool_calls.len());
            for tc in &tool_calls {
                let safe = if let Some(tool) = self.agent.tools().get(&tc.name).await {
                    redact_params(&tc.arguments, tool.sensitive_params())
                } else {
                    tc.arguments.clone()
                };
                redacted_args.push(safe);
            }
            // ADR-148 R2: pre-sanitise each rationale via the unified
            // Layer-B egress gate *before* taking the session lock — the
            // gate is async and the session mutex must not be held across
            // an `.await` (deadlock risk + Send bounds).
            let mut sanitized_rationales: Vec<Option<String>> =
                Vec::with_capacity(tool_calls.len());
            for tc in &tool_calls {
                let s = match tc.reasoning.as_ref() {
                    Some(r) => Some(
                        crate::safety::egress::sanitize_tool_output_via_egress(
                            self.agent.safety(),
                            "tool_rationale",
                            r,
                        )
                        .await,
                    ),
                    None => None,
                };
                sanitized_rationales.push(s);
            }
            let mut sess = self.session.lock().await;
            if let Some(thread) = sess.threads.get_mut(&self.thread_id)
                && let Some(turn) = thread.last_turn_mut()
            {
                // Set turn-level narrative.
                if turn.narrative.is_none() {
                    turn.narrative = narrative;
                }
                for ((tc, safe_args), sanitized_rationale) in tool_calls
                    .iter()
                    .zip(redacted_args)
                    .zip(sanitized_rationales)
                {
                    turn.record_tool_call_with_reasoning(
                        &tc.name,
                        safe_args,
                        sanitized_rationale,
                        Some(tc.id.clone()),
                    );
                }
            }
        }

        // === Phase 1: Preflight (sequential) ===
        // Walk tool_calls checking approval and hooks. Classify
        // each tool as Rejected (by hook) or Runnable. Stop at the
        // first tool that needs approval.
        let mut preflight: Vec<(crate::llm::ToolCall, PreflightOutcome)> = Vec::new();
        let mut runnable: Vec<(usize, crate::llm::ToolCall)> = Vec::new();
        let mut approval_needed: Option<(
            usize,
            crate::llm::ToolCall,
            Arc<dyn crate::tools::Tool>,
            bool, // allow_always
        )> = None;

        // Plan mode: check whether we should intercept write tools.
        let is_plan_mode = {
            let sess = self.session.lock().await;
            sess.threads
                .get(&self.thread_id)
                .is_some_and(|t| t.is_plan_mode())
        };

        for (idx, original_tc) in tool_calls.iter().enumerate() {
            let mut tc = original_tc.clone();

            // Plan mode interception: non-readonly tools get a dry-run preview
            // instead of actual execution.
            if is_plan_mode && let Some(tool) = self.agent.tools().get(&tc.name).await {
                let risk = tool.risk_level_for(&tc.arguments);
                if risk > crate::tools::RiskLevel::Low {
                    preflight.push((
                        tc,
                        PreflightOutcome::Rejected(format!(
                            "[Plan Mode — dry-run] Tool '{}' would execute with risk level '{}'. \
                                 Approve the plan to execute for real.",
                            original_tc.name, risk
                        )),
                    ));
                    continue;
                }
            }

            if is_tool_disabled_by_extensions(&tc.name, self.disabled_extensions) {
                preflight.push((
                    tc,
                    PreflightOutcome::Rejected(
                        "Tool is disabled by extension policy for this session".to_string(),
                    ),
                ));
                continue;
            }

            let tool_opt = self.agent.tools().get(&tc.name).await;
            let sensitive = tool_opt
                .as_ref()
                .map(|t| t.sensitive_params())
                .unwrap_or(&[]);

            // Hook: BeforeToolCall
            let hook_params = redact_params(&tc.arguments, sensitive);
            let event = dasclaw_hooks::HookEvent::ToolCall {
                tool_name: tc.name.clone(),
                parameters: hook_params,
                user_id: self.message.user_id.clone(),
                context: "chat".to_string(),
            };
            match self.agent.hooks().run(&event).await {
                Err(dasclaw_hooks::HookError::Rejected { reason }) => {
                    preflight.push((
                        tc,
                        PreflightOutcome::Rejected(format!(
                            "Tool call rejected by hook: {}",
                            reason
                        )),
                    ));
                    continue;
                }
                Err(err) => {
                    preflight.push((
                        tc,
                        PreflightOutcome::Rejected(format!(
                            "Tool call blocked by hook policy: {}",
                            err
                        )),
                    ));
                    continue;
                }
                Ok(dasclaw_hooks::HookOutcome::Continue {
                    modified: Some(new_params),
                }) => match serde_json::from_str::<serde_json::Value>(&new_params) {
                    Ok(mut parsed) => {
                        if let Some(obj) = parsed.as_object_mut() {
                            for key in sensitive {
                                if let Some(orig_val) = original_tc.arguments.get(*key) {
                                    obj.insert((*key).to_string(), orig_val.clone());
                                }
                            }
                        }
                        tc.arguments = parsed;
                    }
                    Err(e) => {
                        tracing::warn!(
                            tool = %tc.name,
                            "Hook returned non-JSON modification for ToolCall, ignoring: {}",
                            e
                        );
                    }
                },
                _ => {}
            }

            // Check if tool requires approval
            if !self.agent.config.auto_approve_tools
                && let Some(tool) = tool_opt
            {
                use crate::tools::ApprovalRequirement;
                let requirement = tool.requires_approval(&tc.arguments);
                let needs_approval = match requirement {
                    ApprovalRequirement::Never => false,
                    ApprovalRequirement::UnlessAutoApproved => {
                        let sess = self.session.lock().await;
                        !sess.is_tool_auto_approved(&tc.name)
                    }
                    ApprovalRequirement::Always => true,
                };

                if needs_approval {
                    // In non-DM relay channels, auto-deny approval-
                    // requiring tools to prevent stuck AwaitingApproval
                    // state and prompt injection from other users.
                    let is_relay = self.message.channel.ends_with("-relay");
                    let is_dm = self
                        .message
                        .metadata
                        .get("event_type")
                        .and_then(|v| v.as_str())
                        == Some("direct_message");
                    if is_relay && !is_dm {
                        tracing::info!(
                            tool = %tc.name,
                            channel = %self.message.channel,
                            "Auto-denying approval-requiring tool in non-DM relay channel"
                        );
                        let reject_msg = format!(
                            "Tool '{}' requires approval and cannot run in shared channels. \
                             Ask the user to message me directly (DM) to use this tool.",
                            tc.name
                        );
                        preflight.push((tc, PreflightOutcome::Rejected(reject_msg)));
                        continue;
                    }

                    let allow_always = !matches!(requirement, ApprovalRequirement::Always);
                    approval_needed = Some((idx, tc, tool, allow_always));
                    break;
                }
            }

            let preflight_idx = preflight.len();
            preflight.push((tc.clone(), PreflightOutcome::Runnable));
            runnable.push((preflight_idx, tc));
        }

        // === Phase 2: Parallel execution ===
        let mut exec_results: Vec<Option<Result<String, Error>>> =
            (0..preflight.len()).map(|_| None).collect();

        if runnable.len() <= 1 {
            for (pf_idx, tc) in &runnable {
                let tool_meta = crate::channels::tool_enriched_metadata(
                    &self.message.metadata,
                    &tc.id,
                    Some(&tc.arguments),
                );
                let _ = self
                    .agent
                    .channels
                    .send_status(
                        &self.message.channel,
                        StatusUpdate::ToolStarted {
                            name: tc.name.clone(),
                        },
                        &tool_meta,
                    )
                    .await;

                let result = {
                    let mut job_ctx = self.job_ctx.clone();
                    self.agent
                        .execute_chat_tool(&tc.name, &tc.arguments, &mut job_ctx)
                        .await
                };

                let disp_tool = self.agent.tools().get(&tc.name).await;
                let _ = self
                    .agent
                    .channels
                    .send_status(
                        &self.message.channel,
                        crate::channels::status::build_tool_completed(
                            tc.name.clone(),
                            &result,
                            &tc.arguments,
                            disp_tool.as_deref(),
                        ),
                        &tool_meta,
                    )
                    .await;

                exec_results[*pf_idx] = Some(result);
            }
        } else {
            let mut join_set = JoinSet::new();

            for (pf_idx, tc) in &runnable {
                let pf_idx = *pf_idx;
                let tools = self.agent.tools().clone();
                let safety = self.agent.safety().clone();
                let channels = self.agent.channels.clone();
                let job_ctx = self.job_ctx.clone();
                let tc = tc.clone();
                let channel = self.message.channel.clone();
                let metadata = crate::channels::tool_enriched_metadata(
                    &self.message.metadata,
                    &tc.id,
                    Some(&tc.arguments),
                );

                join_set.spawn(async move {
                    let mut job_ctx = job_ctx;
                    let _ = channels
                        .send_status(
                            &channel,
                            StatusUpdate::ToolStarted {
                                name: tc.name.clone(),
                            },
                            &metadata,
                        )
                        .await;

                    let result = execute_chat_tool_standalone(
                        &tools,
                        &safety,
                        &tc.name,
                        &tc.arguments,
                        &mut job_ctx,
                    )
                    .await;

                    let par_tool = tools.get(&tc.name).await;
                    let _ = channels
                        .send_status(
                            &channel,
                            crate::channels::status::build_tool_completed(
                                tc.name.clone(),
                                &result,
                                &tc.arguments,
                                par_tool.as_deref(),
                            ),
                            &metadata,
                        )
                        .await;

                    (pf_idx, result)
                });
            }

            while let Some(join_result) = join_set.join_next().await {
                match join_result {
                    Ok((pf_idx, result)) => {
                        exec_results[pf_idx] = Some(result);
                    }
                    Err(e) => {
                        if e.is_panic() {
                            tracing::error!("Chat tool execution task panicked: {}", e);
                        } else {
                            tracing::error!("Chat tool execution task cancelled: {}", e);
                        }
                    }
                }
            }

            // Fill panicked slots with error results
            for (pf_idx, tc) in runnable.iter() {
                if exec_results[*pf_idx].is_none() {
                    tracing::error!(
                        tool = %tc.name,
                        "Filling failed task slot with error"
                    );
                    exec_results[*pf_idx] = Some(Err(crate::error::ToolError::ExecutionFailed {
                        name: tc.name.clone(),
                        reason: "Task failed during execution".to_string(),
                    }
                    .into()));
                }
            }
        }

        // === Phase 3: Post-flight (sequential, in original order) ===
        let mut deferred_auth: Option<String> = None;

        for (pf_idx, (tc, outcome)) in preflight.into_iter().enumerate() {
            match outcome {
                PreflightOutcome::Rejected(error_msg) => {
                    let sanitized = preflight_rejection_tool_message(
                        self.agent.safety(),
                        &tc.name,
                        &tc.id,
                        &error_msg,
                    );
                    {
                        let mut sess = self.session.lock().await;
                        if let Some(thread) = sess.threads.get_mut(&self.thread_id)
                            && let Some(turn) = thread.last_turn_mut()
                        {
                            turn.record_tool_error_for(&tc.id, sanitized.display.clone());
                        }
                    }
                    reason_ctx.messages.push(sanitized.message);
                }
                PreflightOutcome::Runnable => {
                    let tool_result = exec_results[pf_idx].take().unwrap_or_else(|| {
                        Err(crate::error::ToolError::ExecutionFailed {
                            name: tc.name.clone(),
                            reason: "No result available".to_string(),
                        }
                        .into())
                    });

                    // Detect image generation sentinel
                    let is_image_sentinel = if let Ok(ref output) = tool_result
                        && matches!(tc.name.as_str(), "image_generate" | "image_edit")
                    {
                        if let Ok(sentinel) = serde_json::from_str::<serde_json::Value>(output)
                            && sentinel.get("type").and_then(|v| v.as_str())
                                == Some("image_generated")
                        {
                            let data_url = sentinel
                                .get("data")
                                .and_then(|v| v.as_str())
                                .unwrap_or_default()
                                .to_string();
                            let path = sentinel
                                .get("path")
                                .and_then(|v| v.as_str())
                                .map(String::from);
                            if data_url.is_empty() {
                                tracing::warn!(
                                    "Image generation sentinel has empty data URL, skipping broadcast"
                                );
                            } else {
                                let _ = self
                                    .agent
                                    .channels
                                    .send_status(
                                        &self.message.channel,
                                        StatusUpdate::ImageGenerated { data_url, path },
                                        &self.message.metadata,
                                    )
                                    .await;
                            }
                            true
                        } else {
                            false
                        }
                    } else {
                        false
                    };

                    // Sanitize first so previews, stash, thread record and the
                    // LLM ChatMessage all share the same redacted text. P0-G/W6
                    // (#93): raw tool output must never reach previews or stash.
                    let is_tool_error = tool_result.is_err();
                    let sanitized = crate::tools::execute::process_tool_result(
                        self.agent.safety(),
                        &tc.name,
                        &tc.id,
                        &tool_result,
                    );

                    // Send ToolResult preview using the sanitized display text.
                    if !is_image_sentinel && !is_tool_error && !sanitized.display.is_empty() {
                        let result_meta = crate::channels::tool_enriched_metadata(
                            &self.message.metadata,
                            &tc.id,
                            None,
                        );
                        let _ = self
                            .agent
                            .channels
                            .send_status(
                                &self.message.channel,
                                StatusUpdate::ToolResult {
                                    name: tc.name.clone(),
                                    preview: sanitized.display.clone(),
                                },
                                &result_meta,
                            )
                            .await;
                    }

                    // Check for auth awaiting. Auth detection inspects the raw
                    // tool result for non-secret structural fields (auth_url /
                    // setup_url / instruction text) and never forwards it to
                    // UI/stash, so it is safe to keep on the unsanitized value.
                    if deferred_auth.is_none()
                        && let Some((ext_name, instructions)) =
                            check_auth_required(&tc.name, &tool_result)
                    {
                        let auth_data = parse_auth_result(&tool_result);
                        {
                            let mut sess = self.session.lock().await;
                            if let Some(thread) = sess.threads.get_mut(&self.thread_id) {
                                thread.enter_auth_mode(ext_name.clone());
                            }
                        }
                        let _ = self
                            .agent
                            .channels
                            .send_status(
                                &self.message.channel,
                                StatusUpdate::AuthRequired {
                                    extension_name: ext_name,
                                    instructions: Some(instructions.clone()),
                                    auth_url: auth_data.auth_url,
                                    setup_url: auth_data.setup_url,
                                },
                                &self.message.metadata,
                            )
                            .await;
                        deferred_auth = Some(instructions);
                    }

                    // Stash the sanitized **untruncated** output so subsequent
                    // tools that reference it (e.g. the `json` tool via
                    // `source_tool_call_id`) can still parse the complete
                    // structured payload after the LLM-facing copy has been
                    // capped at `max_output_length`. `stash_content` shares
                    // the same redaction pass as `display`, so no raw
                    // secrets leak through this path.
                    if !is_tool_error {
                        self.job_ctx
                            .tool_output_stash
                            .write()
                            .await
                            .insert(tc.id.clone(), sanitized.stash_content.clone());
                    }

                    let result_content = sanitized.display;
                    let tool_message = sanitized.message;

                    // Record sanitized result in thread (identity-based matching).
                    {
                        let mut sess = self.session.lock().await;
                        if let Some(thread) = sess.threads.get_mut(&self.thread_id)
                            && let Some(turn) = thread.last_turn_mut()
                        {
                            if is_tool_error {
                                turn.record_tool_error_for(&tc.id, result_content.clone());
                            } else {
                                turn.record_tool_result_for(
                                    &tc.id,
                                    serde_json::json!(result_content),
                                );
                            }
                        }
                    }

                    reason_ctx.messages.push(tool_message);
                }
            }
        }

        // Return auth response after all results are recorded
        if let Some(instructions) = deferred_auth {
            return Ok(Some(LoopOutcome::Response(instructions)));
        }

        // Handle approval if a tool needed it
        if let Some((approval_idx, tc, tool, allow_always)) = approval_needed {
            let display_params = redact_params(&tc.arguments, tool.sensitive_params());
            let pending = PendingApproval {
                request_id: Uuid::new_v4(),
                tool_name: tc.name.clone(),
                parameters: tc.arguments.clone(),
                display_parameters: display_params,
                description: tool.description().to_string(),
                tool_call_id: tc.id.clone(),
                context_messages: reason_ctx.messages.clone(),
                deferred_tool_calls: tool_calls[approval_idx + 1..].to_vec(),
                user_timezone: Some(self.user_tz.name().to_string()),
                allow_always,
            };

            return Ok(Some(LoopOutcome::NeedApproval(Box::new(pending))));
        }

        Ok(None)
    }
}
