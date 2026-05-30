//! Worker runtime: the main execution loop inside a container.
//!
//! Reuses the existing `Reasoning` and `SafetyLayer` infrastructure but
//! connects to the orchestrator for LLM calls instead of calling APIs directly.
//! Streams real-time events (message, tool_use, tool_result, result) through
//! the orchestrator's job event pipeline for UI visibility.
//!
//! Drives the shared `dasclaw_runtime::AgenticLoop` via `ContainerResponder`
//! (LLM seam) and `ContainerDispatcher` (tool seam) — ADR-160 §3 L2.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::config::SafetyConfig;
use crate::error::WorkerError;
use crate::llm::{
    ChatMessage, LlmProvider, Reasoning, ReasoningContext, RespondOutput, ResponseMetadata,
};
use crate::safety::SafetyLayer;
use crate::tools::ToolRegistry;
use crate::tools::execute::{execute_tool_simple, process_tool_result};
use crate::worker::api::{CompletionReport, JobEventPayload, StatusUpdate, WorkerHttpClient};
use crate::worker::autonomous_recovery::{
    AutonomousRecoveryAction, AutonomousRecoveryState, EMPTY_TOOL_COMPLETION_FAILURE,
    EMPTY_TOOL_COMPLETION_NUDGE, FORCE_TEXT_RECOVERY_PROMPT,
};
use crate::worker::proxy_llm::ProxyLlmProvider;
use dasclaw_core::TokenUsage;
use dasclaw_core::agentic_loop::AgenticLoopConfig;
use dasclaw_core::intent::truncate_for_preview;
use dasclaw_core::messages::ToolCall;
use dasclaw_core::traits::HostError;
use dasclaw_runtime::context::JobContext;
use dasclaw_runtime::tool_dispatch::ToolDispatcher;
use dasclaw_runtime::{AgentResponder, AgenticLoop, LoopOutcome, TextAction};

/// Configuration for the worker runtime.
pub struct WorkerConfig {
    pub job_id: Uuid,
    pub orchestrator_url: String,
    pub max_iterations: u32,
    pub timeout: Duration,
}

impl Default for WorkerConfig {
    fn default() -> Self {
        Self {
            job_id: Uuid::nil(),
            orchestrator_url: String::new(),
            max_iterations: 50,
            timeout: Duration::from_secs(600),
        }
    }
}

/// The worker runtime runs inside a Docker container.
///
/// It connects to the orchestrator over HTTP, fetches its job description,
/// then runs a tool execution loop until the job is complete. Events are
/// streamed to the orchestrator so the UI can show real-time progress.
pub struct WorkerRuntime {
    config: WorkerConfig,
    client: Arc<WorkerHttpClient>,
    llm: Arc<dyn LlmProvider>,
    safety: Arc<SafetyLayer>,
    tools: Arc<ToolRegistry>,
    /// Credentials fetched from the orchestrator, injected into child processes
    /// via `Command::envs()` rather than mutating the global process environment.
    ///
    /// Wrapped in `Arc` to avoid deep-cloning the map on every tool invocation.
    extra_env: Arc<HashMap<String, String>>,
}

impl WorkerRuntime {
    /// Create a new worker runtime.
    ///
    /// Reads `IRONCLAW_WORKER_TOKEN` from the environment for auth.
    pub async fn new(config: WorkerConfig) -> Result<Self, WorkerError> {
        let client = Arc::new(WorkerHttpClient::from_env(
            config.orchestrator_url.clone(),
            config.job_id,
        )?);

        let llm: Arc<dyn LlmProvider> = Arc::new(ProxyLlmProvider::new(
            Arc::clone(&client),
            "proxied".to_string(),
        ));

        let safety = Arc::new(SafetyLayer::new(&SafetyConfig {
            max_output_length: 100_000,
            injection_check_enabled: true,
        }));

        let tools = Arc::new(ToolRegistry::new());
        // Sandboxed worker: register the full container-domain tool set
        // (filesystem, shell, dev tools) via the unified bootstrap API.
        tools
            .bootstrap_tools(&crate::tools::bootstrap::BootstrapContext {
                mode: crate::tools::bootstrap::BootstrapMode::Container,
                ..Default::default()
            })
            .await
            .map_err(|e| WorkerError::ExecutionFailed {
                reason: format!("bootstrap_tools failed: {e}"),
            })?;

        Ok(Self {
            config,
            client,
            llm,
            safety,
            tools,
            extra_env: Arc::new(HashMap::new()),
        })
    }

    /// Run the worker until the job is complete or an error occurs.
    pub async fn run(mut self) -> Result<(), WorkerError> {
        tracing::info!("Worker starting for job {}", self.config.job_id);

        // Fetch job description from orchestrator
        let job = self.client.get_job().await?;

        tracing::info!(
            "Received job: {} - {}",
            job.title,
            truncate_for_preview(&job.description, 100)
        );

        // Fetch credentials and store them for injection into child processes
        // via Command::envs() (avoids unsafe std::env::set_var in multi-threaded runtime).
        let credentials = self.client.fetch_credentials().await?;
        {
            let mut env_map = HashMap::new();
            for cred in &credentials {
                env_map.insert(cred.env_var.clone(), cred.value.clone());
            }
            self.extra_env = Arc::new(env_map);
        }
        if !credentials.is_empty() {
            tracing::info!(
                "Fetched {} credential(s) for child process injection",
                credentials.len()
            );
        }

        // Report that we're starting
        self.client
            .report_status(&StatusUpdate {
                state: "in_progress".to_string(),
                message: Some("Worker started, beginning execution".to_string()),
                iteration: 0,
            })
            .await?;

        // Reasoning engine is built and moved into the delegate below; the
        // agentic loop engine itself is LLM-type-agnostic (Route B, D-4.5).
        let reasoning = Reasoning::new(self.llm.clone());

        // Build initial context
        let mut reason_ctx = ReasoningContext::new().with_job(&job.description);

        reason_ctx.messages.push(ChatMessage::system(format!(
            r#"You are an autonomous agent running inside a Docker container.

Job: {}
Description: {}

You have tools for shell commands, file operations, and code editing.
Work independently to complete this job. When finished, your final message MUST include the phrase "The job is complete" to signal termination."#,
            job.title, job.description
        )));

        // Load tool definitions.
        // ADR-149 / issue #485 — L2 gate; container worker → Container env.
        reason_ctx.available_tools = self
            .tools
            .tool_definitions_for_llm(
                &dasclaw_governance::tool_visibility::ToolGateContextSeed::system(
                    dasclaw_governance::tool_visibility::Env::Container,
                ),
            )
            .await;

        // Shared iteration tracker — read after the loop to report accurate counts.
        let iteration_tracker = Arc::new(Mutex::new(0u32));

        // Issue #73 slice E: open the workspace capability before the
        // timeout block so `?` propagates cleanly via this fn's error type.
        let workspace_cap = Arc::new(
            dasclaw_workspace_cap::WorkspaceCapability::open(
                std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")),
            )
            .map_err(|e| crate::error::WorkerError::ExecutionFailed {
                reason: format!("failed to open workspace capability: {e}"),
            })?,
        );

        // Run with timeout using the shared agentic loop
        let last_output = Arc::new(Mutex::new(String::new()));
        let recovery_state = Arc::new(Mutex::new(AutonomousRecoveryState::default()));

        let result = tokio::time::timeout(self.config.timeout, async {
            let responder = ContainerResponder {
                client: self.client.clone(),
                tools: self.tools.clone(),
                last_output: last_output.clone(),
                iteration_tracker: iteration_tracker.clone(),
                recovery_state: recovery_state.clone(),
                reasoning,
            };

            let dispatcher = ContainerDispatcher {
                client: self.client.clone(),
                safety: self.safety.clone(),
                tools: self.tools.clone(),
                extra_env: self.extra_env.clone(),
                last_output: last_output.clone(),
                recovery_state: recovery_state.clone(),
            };

            let config = AgenticLoopConfig {
                max_iterations: self.config.max_iterations as usize,
                enable_tool_intent_nudge: true,
                max_tool_intent_nudges: 2,
            };

            // Phase 3 Step G: SafetyLayer wired in via IronclawSafetyHook.
            // sandbox/secrets/approval still default; container worker is
            // already inside Docker (no nested sandbox needed) and runs
            // unattended (auto-approve).
            //
            // Issue #73 slice D: PermissionMode threaded explicitly.
            // Issue #73 slice E: workspace boundary is capability-validated;
            // container workers run inside an isolated Docker FS so the
            // capability handle is over the in-container cwd.
            let hooks = crate::agent::hook_bundle::hook_bundle_with_safety(
                self.safety.clone(),
                workspace_cap.clone(),
                dasclaw_core::permissions::PermissionMode::WorkspaceWrite,
            );

            // ADR-160 §3 L2: drive the shared engine with responder + dispatcher.
            // No cancellation token — container worker lifecycle is owned by the
            // orchestrator (check_signals always Continue). No event channel —
            // events are streamed via WorkerHttpClient::post_event instead.
            AgenticLoop::new(Arc::new(responder), Some(Arc::new(dispatcher)), None, None)
                .run(&mut reason_ctx, &config, &hooks)
                .await
        })
        .await;

        let iterations = *iteration_tracker.lock().await;

        match result {
            Ok(Ok(LoopOutcome::Response(output))) => {
                tracing::info!("Worker completed job {} successfully", self.config.job_id);
                self.post_event(
                    "result",
                    serde_json::json!({
                        "success": true,
                        "message": truncate_for_preview(&output, 2000),
                    }),
                )
                .await;
                self.client
                    .report_complete(&CompletionReport {
                        success: true,
                        message: Some(output),
                        iterations,
                    })
                    .await?;
            }
            Ok(Ok(LoopOutcome::MaxIterations)) => {
                let msg = format!("max iterations ({}) exceeded", self.config.max_iterations);
                tracing::warn!("Worker failed for job {}: {}", self.config.job_id, msg);
                self.post_event(
                    "result",
                    serde_json::json!({
                        "success": false,
                        "message": format!("Execution failed: {}", msg),
                    }),
                )
                .await;
                self.client
                    .report_complete(&CompletionReport {
                        success: false,
                        message: Some(format!("Execution failed: {}", msg)),
                        iterations,
                    })
                    .await?;
            }
            Ok(Ok(LoopOutcome::Failure(reason))) => {
                tracing::warn!("Worker failed for job {}: {}", self.config.job_id, reason);
                self.post_event(
                    "result",
                    serde_json::json!({
                        "success": false,
                        "message": reason,
                    }),
                )
                .await;
                self.client
                    .report_complete(&CompletionReport {
                        success: false,
                        message: Some(reason),
                        iterations,
                    })
                    .await?;
            }
            Ok(Ok(LoopOutcome::Stopped | LoopOutcome::NeedApproval(_))) => {
                tracing::info!("Worker for job {} stopped", self.config.job_id);
                self.client
                    .report_complete(&CompletionReport {
                        success: false,
                        message: Some("Execution stopped".to_string()),
                        iterations,
                    })
                    .await?;
            }
            Ok(Err(e)) => {
                tracing::error!("Worker failed for job {}: {}", self.config.job_id, e);
                self.post_event(
                    "result",
                    serde_json::json!({
                        "success": false,
                        "message": format!("Execution failed: {}", e),
                    }),
                )
                .await;
                self.client
                    .report_complete(&CompletionReport {
                        success: false,
                        message: Some(format!("Execution failed: {}", e)),
                        iterations,
                    })
                    .await?;
            }
            Err(_) => {
                tracing::warn!("Worker timed out for job {}", self.config.job_id);
                self.post_event(
                    "result",
                    serde_json::json!({
                        "success": false,
                        "message": "Execution timed out",
                    }),
                )
                .await;
                self.client
                    .report_complete(&CompletionReport {
                        success: false,
                        message: Some("Execution timed out".to_string()),
                        iterations,
                    })
                    .await?;
            }
        }

        Ok(())
    }

    /// Post a job event to the orchestrator (fire-and-forget).
    async fn post_event(&self, event_type: &str, data: serde_json::Value) {
        self.client
            .post_event(&JobEventPayload {
                event_type: event_type.to_string(),
                data,
            })
            .await;
    }
}

/// Free helper so both responder and dispatcher can post job events without
/// duplicating the small async wrapper.
async fn post_job_event(client: &WorkerHttpClient, event_type: &str, data: serde_json::Value) {
    client
        .post_event(&JobEventPayload {
            event_type: event_type.to_string(),
            data,
        })
        .await;
}

/// LLM seam for the container worker — ADR-160 §3 L2.
///
/// Owns the `Reasoning` engine and the autonomous-recovery decisions taken on
/// each iteration / text response. Shares `last_output` and `recovery_state`
/// with `ContainerDispatcher` via `Arc<Mutex<_>>`.
struct ContainerResponder {
    client: Arc<WorkerHttpClient>,
    tools: Arc<ToolRegistry>,
    /// Last successful tool output — read here to back-fill the final response
    /// when the model signals completion with an empty text body.
    last_output: Arc<Mutex<String>>,
    /// Tracks the current iteration — shared with the outer `run` method so
    /// `CompletionReport` can include accurate iteration counts.
    iteration_tracker: Arc<Mutex<u32>>,
    /// Shared with `ContainerDispatcher` (Arc<Mutex>): responder runs
    /// `begin_iteration` + `on_text_response`, dispatcher runs `on_valid_tool_call`.
    recovery_state: Arc<Mutex<AutonomousRecoveryState>>,
    reasoning: Reasoning,
}

impl ContainerResponder {
    /// Poll the orchestrator for a follow-up prompt. If one is available,
    /// inject it as a user message into the reasoning context.
    async fn poll_and_inject_prompt(&self, reason_ctx: &mut ReasoningContext) {
        match self.client.poll_prompt().await {
            Ok(Some(prompt)) => {
                tracing::info!(
                    "Received follow-up prompt: {}",
                    truncate_for_preview(&prompt.content, 100)
                );
                post_job_event(
                    &self.client,
                    "message",
                    serde_json::json!({
                        "role": "user",
                        "content": truncate_for_preview(&prompt.content, 2000),
                    }),
                )
                .await;
                reason_ctx.messages.push(ChatMessage::user(&prompt.content));
            }
            Ok(None) => {}
            Err(e) => {
                tracing::debug!("Failed to poll for prompt: {}", e);
            }
        }
    }
}

#[async_trait]
impl AgentResponder for ContainerResponder {
    async fn before_llm_call(
        &self,
        reason_ctx: &mut ReasoningContext,
        iteration: usize,
    ) -> Option<LoopOutcome> {
        let iteration = iteration as u32;
        *self.iteration_tracker.lock().await = iteration;

        // Report progress every 5 iterations
        if iteration % 5 == 1 {
            let _ = self
                .client
                .report_status(&StatusUpdate {
                    state: "in_progress".to_string(),
                    message: Some(format!("Iteration {}", iteration)),
                    iteration,
                })
                .await;
        }

        // Poll for follow-up prompts from the user
        self.poll_and_inject_prompt(reason_ctx).await;

        // Claude 4.6 rejects assistant prefill; NEAR AI rejects any non-user-ending
        // conversation. Ensure the last message is user-role before calling the LLM.
        crate::util::ensure_ends_with_user_message(&mut reason_ctx.messages);

        let force_text_recovery = {
            let mut recovery = self.recovery_state.lock().await;
            recovery.begin_iteration()
        };
        if force_text_recovery {
            tracing::warn!("Switching to text-only recovery after malformed tool completions");
            reason_ctx.available_tools.clear();
        } else {
            // Refresh tools (in case WASM tools were built).
            // ADR-149 / issue #485 — L2 gate; container worker → Container env.
            reason_ctx.available_tools = self
                .tools
                .tool_definitions_for_llm(
                    &dasclaw_governance::tool_visibility::ToolGateContextSeed::system(
                        dasclaw_governance::tool_visibility::Env::Container,
                    ),
                )
                .await;
        }

        None
    }

    async fn respond(&self, reason_ctx: &mut ReasoningContext) -> Result<RespondOutput, HostError> {
        // Container uses respond_with_tools (which may return either text or tool calls)
        self.reasoning
            .respond_with_tools(reason_ctx)
            .await
            .map_err(Into::into)
    }

    async fn handle_text_response(
        &self,
        text: &str,
        metadata: ResponseMetadata,
        usage: TokenUsage,
        reason_ctx: &mut ReasoningContext,
    ) -> TextAction {
        let action = {
            let mut recovery = self.recovery_state.lock().await;
            recovery.on_text_response(metadata, text)
        };
        match action {
            AutonomousRecoveryAction::ToolModeNudge => {
                tracing::warn!("Malformed empty tool completion detected; retrying in tool mode");
                post_job_event(
                    &self.client,
                    "status",
                    serde_json::json!({
                        "message": "Model returned an empty tool-completion response; retrying with a stronger tool-use nudge.",
                    }),
                )
                .await;
                reason_ctx
                    .messages
                    .push(ChatMessage::user(EMPTY_TOOL_COMPLETION_NUDGE));
                return TextAction::Continue;
            }
            AutonomousRecoveryAction::ForceTextRecovery => {
                tracing::warn!(
                    "Repeated malformed tool completions detected; switching to text-only recovery"
                );
                post_job_event(
                    &self.client,
                    "status",
                    serde_json::json!({
                        "message": "Model returned repeated empty tool-completion responses; requesting a final status update without tools.",
                    }),
                )
                .await;
                reason_ctx
                    .messages
                    .push(ChatMessage::user(FORCE_TEXT_RECOVERY_PROMPT));
                return TextAction::Continue;
            }
            AutonomousRecoveryAction::Fail => {
                tracing::warn!("Failing fast after repeated malformed autonomous responses");
                return TextAction::Return(LoopOutcome::Failure(
                    EMPTY_TOOL_COMPLETION_FAILURE.to_string(),
                ));
            }
            AutonomousRecoveryAction::Continue => {}
        }

        post_job_event(
            &self.client,
            "message",
            serde_json::json!({
                "role": "assistant",
                "content": truncate_for_preview(text, 2000),
            }),
        )
        .await;

        // Check for completion
        if crate::util::llm_signals_completion(text) {
            let last = self.last_output.lock().await;
            let output = if last.is_empty() {
                text.to_string()
            } else {
                last.clone()
            };
            return TextAction::Return(LoopOutcome::Response(output));
        }

        reason_ctx
            .messages
            .push(ChatMessage::assistant(text).with_usage(usage));
        TextAction::Continue
    }

    async fn on_tool_intent_nudge(&self, text: &str, _reason_ctx: &mut ReasoningContext) {
        post_job_event(
            &self.client,
            "message",
            serde_json::json!({
                "role": "assistant",
                "content": truncate_for_preview(text, 2000),
                "nudge": true,
            }),
        )
        .await;
    }

    async fn after_iteration(&self, _iteration: usize) {
        // Brief pause between iterations
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

/// Tool seam for the container worker — ADR-160 §3 L2.
///
/// Executes tools sequentially (no parallel execution in container context)
/// and streams `tool_use` / `tool_result` events back to the orchestrator.
/// Shares `last_output` and `recovery_state` with `ContainerResponder`.
struct ContainerDispatcher {
    client: Arc<WorkerHttpClient>,
    safety: Arc<SafetyLayer>,
    tools: Arc<ToolRegistry>,
    extra_env: Arc<HashMap<String, String>>,
    last_output: Arc<Mutex<String>>,
    recovery_state: Arc<Mutex<AutonomousRecoveryState>>,
}

#[async_trait]
impl ToolDispatcher for ContainerDispatcher {
    async fn dispatch(
        &self,
        tool_calls: Vec<ToolCall>,
        content: Option<String>,
        usage: TokenUsage,
        reason_ctx: &mut ReasoningContext,
    ) -> Result<Option<LoopOutcome>, HostError> {
        {
            let mut recovery = self.recovery_state.lock().await;
            recovery.on_valid_tool_call();
        }

        if let Some(ref text) = content {
            post_job_event(
                &self.client,
                "message",
                serde_json::json!({
                    "role": "assistant",
                    "content": truncate_for_preview(text, 2000),
                }),
            )
            .await;
        }

        // Add assistant message with tool_calls (OpenAI protocol)
        reason_ctx.messages.push(
            ChatMessage::assistant_with_tool_calls(content, tool_calls.clone()).with_usage(usage),
        );

        // Execute tools sequentially (container context — no parallel execution)
        for tc in tool_calls {
            post_job_event(
                &self.client,
                "tool_use",
                serde_json::json!({
                    "tool_name": tc.name,
                    "input": truncate_for_preview(&tc.arguments.to_string(), 500),
                }),
            )
            .await;

            let mut job_ctx = JobContext {
                extra_env: self.extra_env.clone(),
                ..Default::default()
            };

            let result = execute_tool_simple(
                &self.tools,
                &self.safety,
                &tc.name,
                tc.arguments.clone(),
                &mut job_ctx,
            )
            .await;

            post_job_event(
                &self.client,
                "tool_result",
                serde_json::json!({
                    "tool_name": tc.name,
                    "output": match &result {
                        Ok(output) => truncate_for_preview(output, 2000),
                        Err(e) => format!("Error: {}", truncate_for_preview(e, 500)).into(),
                    },
                    "success": result.is_ok(),
                }),
            )
            .await;

            if let Ok(ref output) = result {
                *self.last_output.lock().await = output.clone();
            }

            // Use shared result processing
            let sanitized = process_tool_result(&self.safety, &tc.name, &tc.id, &result);
            reason_ctx.messages.push(sanitized.message);
        }

        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use dasclaw_core::intent::truncate_for_preview;

    #[test]
    fn test_truncate_within_limit() {
        assert_eq!(truncate_for_preview("hello", 10), "hello");
    }

    #[test]
    fn test_truncate_at_limit() {
        assert_eq!(truncate_for_preview("hello", 5), "hello");
    }

    #[test]
    fn test_truncate_beyond_limit() {
        let result = truncate_for_preview("hello world", 5);
        assert_eq!(result, "hello...");
    }

    #[test]
    fn test_truncate_multibyte_safe() {
        // "é" is 2 bytes in UTF-8; slicing at byte 1 would panic without safety
        let result = truncate_for_preview("é is fancy", 1);
        // Should truncate to 0 chars (can't fit "é" in 1 byte)
        assert_eq!(result, "...");
    }
}
