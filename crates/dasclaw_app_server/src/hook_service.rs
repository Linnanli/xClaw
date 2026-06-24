use std::sync::Mutex;

use dasclaw_app_server_protocol::{
    ConfigWarningNotification, DeprecationNoticeNotification, GuardianWarningNotification,
    HookCompletedNotification, HookEventName, HookExecutionMode, HookHandlerType, HookOutputEntry,
    HookOutputEntryKind, HookRunStatus, HookRunSummary, HookScope, HookSource,
    HookStartedNotification, ServiceHealth, ServiceName, WarningNotification,
};
use dasclaw_hooks::{HookObservedRun, HookObservedStatus, HookRunObserver};

use crate::app_services::HookNotificationService;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppServerHookNotification {
    Started(HookStartedNotification),
    Completed(HookCompletedNotification),
    Warning(WarningNotification),
    GuardianWarning(GuardianWarningNotification),
    ConfigWarning(ConfigWarningNotification),
    DeprecationNotice(DeprecationNoticeNotification),
}

#[derive(Debug, Default)]
pub struct AppServerHookService {
    notifications: Mutex<Vec<AppServerHookNotification>>,
    producer_wired: bool,
}

impl AppServerHookService {
    pub fn unwired() -> Self {
        Self {
            notifications: Mutex::new(Vec::new()),
            producer_wired: false,
        }
    }

    pub fn wired() -> Self {
        Self {
            notifications: Mutex::new(Vec::new()),
            producer_wired: true,
        }
    }

    pub fn push(&self, notification: AppServerHookNotification) {
        let mut notifications = self
            .notifications
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        notifications.push(notification);
    }

    pub fn push_many(&self, pending: impl IntoIterator<Item = AppServerHookNotification>) {
        let mut notifications = self
            .notifications
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        notifications.extend(pending);
    }
}

impl HookRunObserver for AppServerHookService {
    fn hook_started(&self, run: HookObservedRun) {
        if let Some(notification) = observed_started_notification(&run) {
            self.push(AppServerHookNotification::Started(notification));
        }
    }

    fn hook_completed(&self, run: HookObservedRun) {
        if let Some(notification) = observed_completed_notification(&run) {
            self.push(AppServerHookNotification::Completed(notification));
        }
    }
}

impl HookNotificationService for AppServerHookService {
    fn health(&self) -> ServiceHealth {
        if self.producer_wired {
            ServiceHealth::ready(ServiceName::Hooks)
        } else {
            ServiceHealth::disabled(
                ServiceName::Hooks,
                "hook notification producer is not wired",
            )
        }
    }

    fn drain_hook_notifications(&self) -> Vec<AppServerHookNotification> {
        let mut notifications = self
            .notifications
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        std::mem::take(&mut *notifications)
    }

    fn push_hook_notifications(&self, pending: Vec<AppServerHookNotification>) {
        self.push_many(pending);
    }
}

fn observed_started_notification(run: &HookObservedRun) -> Option<HookStartedNotification> {
    Some(HookStartedNotification {
        thread_id: run.thread_id.clone()?,
        turn_id: run.turn_id.clone(),
        run: observed_run_to_summary(run)?,
    })
}

fn observed_completed_notification(run: &HookObservedRun) -> Option<HookCompletedNotification> {
    Some(HookCompletedNotification {
        thread_id: run.thread_id.clone()?,
        turn_id: run.turn_id.clone(),
        run: observed_run_to_summary(run)?,
    })
}

fn observed_run_to_summary(run: &HookObservedRun) -> Option<HookRunSummary> {
    Some(HookRunSummary {
        id: run.id.clone(),
        event_name: observed_event_name(&run.event_name)?,
        handler_type: HookHandlerType::Command,
        execution_mode: HookExecutionMode::Sync,
        scope: if run.turn_id.is_some() {
            HookScope::Turn
        } else {
            HookScope::Thread
        },
        source_path: format!("dasclaw://hook/{}", run.hook_name),
        source: HookSource::Project,
        display_order: 0,
        status: observed_status(run.status),
        status_message: run.status_message.clone(),
        started_at: run.started_at_ms,
        completed_at: run.completed_at_ms,
        duration_ms: run.duration_ms,
        entries: observed_entries(run),
    })
}

fn observed_event_name(event_name: &str) -> Option<HookEventName> {
    Some(match event_name {
        "beforeToolCall" => HookEventName::PreToolUse,
        "beforeInbound" => HookEventName::UserPromptSubmit,
        "beforeOutbound" | "transformResponse" => HookEventName::PostToolUse,
        "onSessionStart" => HookEventName::SessionStart,
        "onSessionEnd" => HookEventName::Stop,
        _ => return None,
    })
}

fn observed_status(status: HookObservedStatus) -> HookRunStatus {
    match status {
        HookObservedStatus::Running => HookRunStatus::Running,
        HookObservedStatus::Completed => HookRunStatus::Completed,
        HookObservedStatus::Rejected => HookRunStatus::Blocked,
        HookObservedStatus::Failed | HookObservedStatus::TimedOut => HookRunStatus::Failed,
    }
}

fn observed_entries(run: &HookObservedRun) -> Vec<HookOutputEntry> {
    let Some(message) = &run.status_message else {
        return Vec::new();
    };

    let kind = match run.status {
        HookObservedStatus::Rejected => HookOutputEntryKind::Stop,
        HookObservedStatus::Failed | HookObservedStatus::TimedOut => HookOutputEntryKind::Error,
        HookObservedStatus::Running | HookObservedStatus::Completed => {
            HookOutputEntryKind::Feedback
        }
    };

    vec![HookOutputEntry {
        kind,
        text: message.clone(),
    }]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn observed_run(event_name: &str, thread_id: Option<&str>) -> HookObservedRun {
        HookObservedRun {
            id: "hook-run-1".to_string(),
            event_name: event_name.to_string(),
            thread_id: thread_id.map(str::to_string),
            turn_id: Some("turn-1".to_string()),
            hook_name: "audit".to_string(),
            status: HookObservedStatus::Running,
            status_message: None,
            started_at_ms: 1,
            completed_at_ms: None,
            duration_ms: None,
        }
    }

    #[test]
    fn hook_observer_skips_runs_without_thread_owner() {
        let service = AppServerHookService::wired();

        service.hook_started(observed_run("beforeToolCall", None));
        service.hook_completed(observed_run("beforeToolCall", None));

        assert!(service.drain_hook_notifications().is_empty());
    }

    #[test]
    fn hook_observer_skips_unknown_event_names() {
        let service = AppServerHookService::wired();

        service.hook_started(observed_run("futureLifecycle", Some("thread-1")));
        service.hook_completed(observed_run("futureLifecycle", Some("thread-1")));

        assert!(service.drain_hook_notifications().is_empty());
    }
}
