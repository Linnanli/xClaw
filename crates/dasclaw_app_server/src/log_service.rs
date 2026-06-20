use std::sync::{Arc, Mutex, mpsc};

use dasclaw_app_server_protocol::{
    DEFAULT_MAX_PENDING_NOTIFICATIONS, LogEntryEvent, LogLevel, ServiceHealth, ServiceName,
};
use dasclaw_observability::{Observer, ObserverEvent, ObserverMetric};

#[derive(Clone)]
pub struct AppServerLogService {
    sender: mpsc::SyncSender<LogEntryEvent>,
    receiver: Arc<Mutex<mpsc::Receiver<LogEntryEvent>>>,
}

impl AppServerLogService {
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::sync_channel(usize::from(DEFAULT_MAX_PENDING_NOTIFICATIONS));
        Self {
            sender,
            receiver: Arc::new(Mutex::new(receiver)),
        }
    }

    pub fn drain_entries(&self) -> Vec<LogEntryEvent> {
        let receiver = self
            .receiver
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let mut entries = Vec::new();
        while let Ok(entry) = receiver.try_recv() {
            entries.push(entry);
        }
        entries
    }

    fn emit(
        &self,
        level: LogLevel,
        target: impl Into<String>,
        message: impl Into<String>,
        fields: serde_json::Value,
    ) {
        let fields = fields.as_object().cloned().unwrap_or_default();
        let _ = self.sender.try_send(LogEntryEvent {
            level,
            target: target.into(),
            message: message.into(),
            time: crate::unix_timestamp_string(),
            fields,
        });
    }
}

impl Default for AppServerLogService {
    fn default() -> Self {
        Self::new()
    }
}

impl crate::app_services::LogService for AppServerLogService {
    fn health(&self) -> ServiceHealth {
        ServiceHealth::ready(ServiceName::Logs)
    }

    fn drain_entries(&self) -> Vec<LogEntryEvent> {
        AppServerLogService::drain_entries(self)
    }
}

impl Observer for AppServerLogService {
    fn record_event(&self, event: &ObserverEvent) {
        match event {
            ObserverEvent::AgentStart { provider, model } => self.emit(
                LogLevel::Info,
                "observability.agent",
                "agent started",
                serde_json::json!({"provider": provider, "model": model}),
            ),
            ObserverEvent::LlmRequest {
                provider,
                model,
                message_count,
            } => self.emit(
                LogLevel::Info,
                "observability.llm",
                "llm request sent",
                serde_json::json!({
                    "provider": provider,
                    "model": model,
                    "messageCount": message_count,
                }),
            ),
            ObserverEvent::LlmResponse {
                provider,
                model,
                duration,
                success,
                error_message,
            } => self.emit(
                if *success {
                    LogLevel::Info
                } else {
                    LogLevel::Warn
                },
                "observability.llm",
                "llm response received",
                serde_json::json!({
                    "provider": provider,
                    "model": model,
                    "durationMs": duration.as_millis() as u64,
                    "success": success,
                    "errorMessage": error_message,
                }),
            ),
            ObserverEvent::ToolCallStart { tool } => self.emit(
                LogLevel::Info,
                "observability.tool",
                "tool call started",
                serde_json::json!({"tool": tool}),
            ),
            ObserverEvent::ToolCallEnd {
                tool,
                duration,
                success,
            } => self.emit(
                if *success {
                    LogLevel::Info
                } else {
                    LogLevel::Warn
                },
                "observability.tool",
                "tool call completed",
                serde_json::json!({
                    "tool": tool,
                    "durationMs": duration.as_millis() as u64,
                    "success": success,
                }),
            ),
            ObserverEvent::TurnComplete => self.emit(
                LogLevel::Info,
                "observability.turn",
                "turn completed",
                serde_json::json!({}),
            ),
            ObserverEvent::ChannelMessage { channel, direction } => self.emit(
                LogLevel::Debug,
                "observability.channel",
                "channel message",
                serde_json::json!({"channel": channel, "direction": direction}),
            ),
            ObserverEvent::HeartbeatTick => self.emit(
                LogLevel::Debug,
                "observability.heartbeat",
                "heartbeat tick",
                serde_json::json!({}),
            ),
            ObserverEvent::AgentEnd {
                duration,
                tokens_used,
            } => self.emit(
                LogLevel::Info,
                "observability.agent",
                "agent ended",
                serde_json::json!({
                    "durationMs": duration.as_millis() as u64,
                    "tokensUsed": tokens_used,
                }),
            ),
            ObserverEvent::Error { component, message } => self.emit(
                LogLevel::Warn,
                "observability.error",
                message,
                serde_json::json!({"component": component}),
            ),
            ObserverEvent::PromptCache {
                cache_read_tokens,
                cache_creation_tokens,
                total_input_tokens,
                static_layer_changed,
            } => self.emit(
                LogLevel::Debug,
                "observability.prompt_cache",
                "prompt cache stats",
                serde_json::json!({
                    "cacheReadTokens": cache_read_tokens,
                    "cacheCreationTokens": cache_creation_tokens,
                    "totalInputTokens": total_input_tokens,
                    "staticLayerChanged": static_layer_changed,
                }),
            ),
        }
    }

    fn record_metric(&self, metric: &ObserverMetric) {
        let (name, value) = match metric {
            ObserverMetric::RequestLatency(duration) => (
                "request_latency_ms",
                serde_json::json!(duration.as_millis() as u64),
            ),
            ObserverMetric::TokensUsed(tokens) => ("tokens_used", serde_json::json!(tokens)),
            ObserverMetric::ActiveJobs(jobs) => ("active_jobs", serde_json::json!(jobs)),
            ObserverMetric::QueueDepth(depth) => ("queue_depth", serde_json::json!(depth)),
            ObserverMetric::PromptCacheHitRate(rate) => {
                ("prompt_cache_hit_rate", serde_json::json!(rate))
            }
        };
        self.emit(
            LogLevel::Debug,
            "observability.metric",
            name,
            serde_json::json!({"value": value}),
        );
    }

    fn name(&self) -> &str {
        "app_server"
    }
}

#[cfg(test)]
mod tests {
    use dasclaw_app_server_protocol::DEFAULT_MAX_PENDING_NOTIFICATIONS;
    use dasclaw_observability::{Observer, ObserverEvent};

    use super::AppServerLogService;

    #[test]
    fn app_server_log_service_bounds_entries_before_notification_bus() {
        let service = AppServerLogService::new();
        for index in 0..usize::from(DEFAULT_MAX_PENDING_NOTIFICATIONS) + 8 {
            service.record_event(&ObserverEvent::Error {
                component: "app_server.tests".to_string(),
                message: format!("test log message {index}"),
            });
        }

        let entries = service.drain_entries();
        assert_eq!(
            entries.len(),
            usize::from(DEFAULT_MAX_PENDING_NOTIFICATIONS),
            "log bridge must not buffer unbounded entries ahead of NotificationBus"
        );
    }
}
