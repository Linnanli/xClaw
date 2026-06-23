use std::sync::Mutex;

use dasclaw_app_server_protocol::{
    ConfigWarningNotification, DeprecationNoticeNotification, GuardianWarningNotification,
    HookCompletedNotification, HookStartedNotification, ServiceHealth, ServiceName,
    WarningNotification,
};

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
}

impl AppServerHookService {
    pub fn push(&self, notification: AppServerHookNotification) {
        let mut notifications = self
            .notifications
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        notifications.push(notification);
    }
}

impl HookNotificationService for AppServerHookService {
    fn health(&self) -> ServiceHealth {
        ServiceHealth::ready(ServiceName::Hooks)
    }

    fn drain_hook_notifications(&self) -> Vec<AppServerHookNotification> {
        let mut notifications = self
            .notifications
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        std::mem::take(&mut *notifications)
    }
}
