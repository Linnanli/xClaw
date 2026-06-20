use std::fmt;
use std::sync::Arc;

use dasclaw_app_server_protocol::{
    AppServerServiceAvailability, JobListParams, JobListResponse, JobReadParams, JobReadResponse,
    ListMcpServerStatusParams, ListMcpServerStatusResponse, LogEntryEvent, McpResourceReadParams,
    McpResourceReadResponse, McpServerOauthLoginParams, McpServerOauthLoginResponse,
    McpServerReloadParams, McpServerReloadResponse, McpServerToolCallParams,
    McpServerToolCallResponse, McpServiceAvailability, McpToolCallProgressNotification,
    ServiceHealth, ServiceName, ServiceStatus, SkillsConfigWriteParams, SkillsConfigWriteResponse,
    SkillsListParams, SkillsListResponse,
};
use dasclaw_runtime::context::ContextManager;

use crate::AppServerError;
use crate::job_service::AppServerJobService;
use crate::log_service::AppServerLogService;
use crate::mcp_service::AppServerMcpService;
use crate::skills_service::AppServerSkillsService;

pub trait LogService: Send + Sync {
    fn health(&self) -> ServiceHealth;
    fn drain_entries(&self) -> Vec<LogEntryEvent>;

    fn is_ready(&self) -> bool {
        self.health().status == ServiceStatus::Ready
    }
}

pub trait JobService: Send + Sync {
    fn health(&self) -> ServiceHealth;
    fn list(&self, params: JobListParams) -> Result<JobListResponse, AppServerError>;
    fn read(&self, params: JobReadParams) -> Result<JobReadResponse, AppServerError>;

    fn is_ready(&self) -> bool {
        self.health().status == ServiceStatus::Ready
    }
}

pub trait SkillsService: Send + Sync {
    fn health(&self) -> ServiceHealth;
    fn list(&self, params: SkillsListParams) -> Result<SkillsListResponse, AppServerError>;
    fn write_config(
        &self,
        params: SkillsConfigWriteParams,
    ) -> Result<SkillsConfigWriteResponse, AppServerError>;

    fn is_ready(&self) -> bool {
        self.health().status == ServiceStatus::Ready
    }
}

pub trait McpService: Send + Sync {
    fn health(&self) -> ServiceHealth;
    fn list_status(
        &self,
        params: ListMcpServerStatusParams,
    ) -> Result<ListMcpServerStatusResponse, AppServerError>;
    fn reload(
        &self,
        params: McpServerReloadParams,
    ) -> Result<McpServerReloadResponse, AppServerError>;
    fn call_tool(
        &self,
        params: McpServerToolCallParams,
    ) -> Result<McpServerToolCallResponse, AppServerError>;
    fn read_resource(
        &self,
        params: McpResourceReadParams,
    ) -> Result<McpResourceReadResponse, AppServerError>;
    fn oauth_login(
        &self,
        params: McpServerOauthLoginParams,
    ) -> Result<McpServerOauthLoginResponse, AppServerError>;
    fn drain_tool_call_progress_events(&self) -> Vec<McpToolCallProgressNotification> {
        Vec::new()
    }

    fn availability(&self) -> McpServiceAvailability {
        McpServiceAvailability::default()
    }

    fn is_ready(&self) -> bool {
        self.health().status == ServiceStatus::Ready
    }
}

#[derive(Clone)]
pub struct AppServerServices {
    pub logs: Arc<dyn LogService>,
    pub jobs: Arc<dyn JobService>,
    pub skills: Arc<dyn SkillsService>,
    pub mcp: Arc<dyn McpService>,
}

impl fmt::Debug for AppServerServices {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AppServerServices")
            .field("availability", &self.availability())
            .finish()
    }
}

impl Default for AppServerServices {
    fn default() -> Self {
        Self {
            logs: Arc::new(NoopLogService),
            jobs: Arc::new(NoopJobService),
            skills: Arc::new(NoopSkillsService),
            mcp: Arc::new(NoopMcpService),
        }
    }
}

impl AppServerServices {
    #[must_use]
    pub fn real() -> Self {
        let manager = Arc::new(ContextManager::default());
        Self {
            logs: Arc::new(AppServerLogService::new()),
            jobs: Arc::new(AppServerJobService::new(manager)),
            skills: Arc::new(AppServerSkillsService::new()),
            mcp: Arc::new(AppServerMcpService::default()),
        }
    }

    pub fn health(&self) -> Vec<ServiceHealth> {
        vec![
            self.logs.health(),
            self.jobs.health(),
            self.skills.health(),
            self.mcp.health(),
        ]
    }

    pub fn drain_log_entries(&self) -> Vec<LogEntryEvent> {
        self.logs.drain_entries()
    }

    pub fn drain_mcp_tool_call_progress_events(&self) -> Vec<McpToolCallProgressNotification> {
        self.mcp.drain_tool_call_progress_events()
    }

    pub fn availability(&self) -> AppServerServiceAvailability {
        AppServerServiceAvailability {
            logs: self.logs.is_ready(),
            jobs: self.jobs.is_ready(),
            skills: self.skills.is_ready(),
            mcp: if self.mcp.is_ready() {
                self.mcp.availability()
            } else {
                McpServiceAvailability::default()
            },
        }
    }

    #[cfg(test)]
    pub fn for_tests(
        logs: impl LogService + 'static,
        jobs: impl JobService + 'static,
        skills: impl SkillsService + 'static,
        mcp: impl McpService + 'static,
    ) -> Self {
        Self {
            logs: Arc::new(logs),
            jobs: Arc::new(jobs),
            skills: Arc::new(skills),
            mcp: Arc::new(mcp),
        }
    }
}

struct NoopLogService;
struct NoopJobService;
struct NoopSkillsService;
struct NoopMcpService;

impl LogService for NoopLogService {
    fn health(&self) -> ServiceHealth {
        ServiceHealth::disabled(ServiceName::Logs, "log source is not wired")
    }

    fn drain_entries(&self) -> Vec<LogEntryEvent> {
        Vec::new()
    }
}

impl JobService for NoopJobService {
    fn health(&self) -> ServiceHealth {
        ServiceHealth::disabled(ServiceName::Jobs, "job host is not wired")
    }

    fn list(&self, _params: JobListParams) -> Result<JobListResponse, AppServerError> {
        Err(AppServerError::capability_unavailable(
            "jobs",
            "job host is not wired",
        ))
    }

    fn read(&self, _params: JobReadParams) -> Result<JobReadResponse, AppServerError> {
        Err(AppServerError::capability_unavailable(
            "jobs",
            "job host is not wired",
        ))
    }
}

impl SkillsService for NoopSkillsService {
    fn health(&self) -> ServiceHealth {
        ServiceHealth::disabled(ServiceName::Skills, "skills registry is not wired")
    }

    fn list(&self, _params: SkillsListParams) -> Result<SkillsListResponse, AppServerError> {
        Err(AppServerError::capability_unavailable(
            "skills",
            "skills registry is not wired",
        ))
    }

    fn write_config(
        &self,
        _params: SkillsConfigWriteParams,
    ) -> Result<SkillsConfigWriteResponse, AppServerError> {
        Err(AppServerError::capability_unavailable(
            "skills",
            "skills registry is not wired",
        ))
    }
}

impl McpService for NoopMcpService {
    fn health(&self) -> ServiceHealth {
        ServiceHealth::disabled(ServiceName::Mcp, "MCP registry is not wired")
    }

    fn list_status(
        &self,
        _params: ListMcpServerStatusParams,
    ) -> Result<ListMcpServerStatusResponse, AppServerError> {
        Err(AppServerError::capability_unavailable(
            "mcp",
            "MCP registry is not wired",
        ))
    }

    fn reload(
        &self,
        _params: McpServerReloadParams,
    ) -> Result<McpServerReloadResponse, AppServerError> {
        Err(AppServerError::capability_unavailable(
            "mcp",
            "MCP registry is not wired",
        ))
    }

    fn call_tool(
        &self,
        _params: McpServerToolCallParams,
    ) -> Result<McpServerToolCallResponse, AppServerError> {
        Err(AppServerError::capability_unavailable(
            "mcp",
            "MCP registry is not wired",
        ))
    }

    fn read_resource(
        &self,
        _params: McpResourceReadParams,
    ) -> Result<McpResourceReadResponse, AppServerError> {
        Err(AppServerError::capability_unavailable(
            "mcp",
            "MCP registry is not wired",
        ))
    }

    fn oauth_login(
        &self,
        _params: McpServerOauthLoginParams,
    ) -> Result<McpServerOauthLoginResponse, AppServerError> {
        Err(AppServerError::capability_unavailable(
            "mcp",
            "MCP registry is not wired",
        ))
    }
}

#[cfg(test)]
pub use test_fakes::{TestJobService, TestLogService, TestMcpService, TestSkillsService};

#[cfg(test)]
mod test_fakes {
    use std::sync::{Arc, Mutex};

    use dasclaw_app_server_protocol::{
        JobListParams, JobListResponse, JobReadParams, JobReadResponse, JobSnapshot,
        ListMcpServerStatusParams, ListMcpServerStatusResponse, LogEntryEvent,
        McpResourceReadParams, McpResourceReadResponse, McpServerOauthLoginParams,
        McpServerOauthLoginResponse, McpServerReloadParams, McpServerReloadResponse,
        McpServerStatus, McpServerToolCallParams, McpServerToolCallResponse,
        McpServiceAvailability, ServiceHealth, ServiceName, SkillsConfigWriteParams,
        SkillsConfigWriteResponse, SkillsListEntry, SkillsListParams, SkillsListResponse,
    };

    use super::{JobService, LogService, McpService, SkillsService};
    use crate::AppServerError;

    #[derive(Clone)]
    pub struct TestLogService {
        ready: bool,
    }

    impl TestLogService {
        pub fn ready() -> Self {
            Self { ready: true }
        }
    }

    impl LogService for TestLogService {
        fn health(&self) -> ServiceHealth {
            if self.ready {
                ServiceHealth::ready(ServiceName::Logs)
            } else {
                ServiceHealth::disabled(ServiceName::Logs, "test log service disabled")
            }
        }

        fn drain_entries(&self) -> Vec<LogEntryEvent> {
            Vec::new()
        }
    }

    #[derive(Clone)]
    pub struct TestJobService {
        jobs: Vec<JobSnapshot>,
    }

    impl TestJobService {
        pub fn ready(jobs: Vec<JobSnapshot>) -> Self {
            Self { jobs }
        }
    }

    impl JobService for TestJobService {
        fn health(&self) -> ServiceHealth {
            ServiceHealth::ready(ServiceName::Jobs)
        }

        fn list(&self, _params: JobListParams) -> Result<JobListResponse, AppServerError> {
            Ok(JobListResponse {
                data: self.jobs.clone(),
                next_cursor: None,
            })
        }

        fn read(&self, params: JobReadParams) -> Result<JobReadResponse, AppServerError> {
            self.jobs
                .iter()
                .find(|job| job.job_id == params.job_id)
                .cloned()
                .map(|job| JobReadResponse { job })
                .ok_or_else(|| AppServerError::invalid_request("jobs", "job not found"))
        }
    }

    #[derive(Clone)]
    pub struct TestSkillsService {
        entries: Arc<Mutex<Vec<SkillsListEntry>>>,
    }

    impl TestSkillsService {
        pub fn ready(entries: Vec<SkillsListEntry>) -> Self {
            Self {
                entries: Arc::new(Mutex::new(entries)),
            }
        }
    }

    impl SkillsService for TestSkillsService {
        fn health(&self) -> ServiceHealth {
            ServiceHealth::ready(ServiceName::Skills)
        }

        fn list(&self, params: SkillsListParams) -> Result<SkillsListResponse, AppServerError> {
            let entries = self
                .entries
                .lock()
                .unwrap_or_else(|poison| poison.into_inner());
            let data = entries
                .iter()
                .filter(|entry| params.cwds.is_empty() || params.cwds.contains(&entry.cwd))
                .cloned()
                .collect();
            Ok(SkillsListResponse { data })
        }

        fn write_config(
            &self,
            params: SkillsConfigWriteParams,
        ) -> Result<SkillsConfigWriteResponse, AppServerError> {
            let mut entries = self
                .entries
                .lock()
                .unwrap_or_else(|poison| poison.into_inner());
            let skill = entries
                .iter_mut()
                .find_map(|entry| {
                    entry.skills.iter_mut().find(|skill| {
                        params.path.as_ref().is_some_and(|path| skill.path == *path)
                            || params.name.as_ref().is_some_and(|name| skill.name == *name)
                    })
                })
                .ok_or_else(|| AppServerError::invalid_request("skills", "skill not found"))?;
            skill.enabled = params.enabled;
            Ok(SkillsConfigWriteResponse {
                effective_enabled: skill.enabled,
            })
        }
    }

    #[derive(Clone)]
    pub struct TestMcpService {
        ready: bool,
        statuses: Vec<McpServerStatus>,
    }

    impl TestMcpService {
        pub fn ready(statuses: Vec<McpServerStatus>) -> Self {
            Self {
                ready: true,
                statuses,
            }
        }

        pub fn disabled_with_advertised_methods(statuses: Vec<McpServerStatus>) -> Self {
            Self {
                ready: false,
                statuses,
            }
        }
    }

    impl McpService for TestMcpService {
        fn health(&self) -> ServiceHealth {
            if self.ready {
                ServiceHealth::ready(ServiceName::Mcp)
            } else {
                ServiceHealth::disabled(ServiceName::Mcp, "test MCP service disabled")
            }
        }

        fn list_status(
            &self,
            _params: ListMcpServerStatusParams,
        ) -> Result<ListMcpServerStatusResponse, AppServerError> {
            Ok(ListMcpServerStatusResponse {
                data: self.statuses.clone(),
                next_cursor: None,
            })
        }

        fn reload(
            &self,
            params: McpServerReloadParams,
        ) -> Result<McpServerReloadResponse, AppServerError> {
            let reloaded = params.name.map(|name| vec![name]).unwrap_or_else(|| {
                self.statuses
                    .iter()
                    .map(|status| status.name.clone())
                    .collect()
            });
            Ok(McpServerReloadResponse { reloaded })
        }

        fn call_tool(
            &self,
            params: McpServerToolCallParams,
        ) -> Result<McpServerToolCallResponse, AppServerError> {
            Ok(McpServerToolCallResponse {
                content: vec![serde_json::json!({
                    "server": params.server,
                    "tool": params.tool,
                    "arguments": params.arguments,
                })],
                structured_content: None,
                is_error: Some(false),
                meta: params.meta,
            })
        }

        fn read_resource(
            &self,
            params: McpResourceReadParams,
        ) -> Result<McpResourceReadResponse, AppServerError> {
            if self
                .statuses
                .iter()
                .any(|status| status.name == params.server)
            {
                Ok(McpResourceReadResponse { contents: vec![] })
            } else {
                Err(AppServerError::capability_unavailable(
                    "mcp",
                    format!("unknown MCP server: {}", params.server),
                ))
            }
        }

        fn oauth_login(
            &self,
            _params: McpServerOauthLoginParams,
        ) -> Result<McpServerOauthLoginResponse, AppServerError> {
            Ok(McpServerOauthLoginResponse {
                authorization_url: "https://auth.example.test/oauth".to_string(),
            })
        }

        fn availability(&self) -> McpServiceAvailability {
            McpServiceAvailability {
                status_list: true,
                reload: true,
                tool_call: true,
                resource_read: true,
                startup_status_events: true,
                ..McpServiceAvailability::default()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::AppServerServices;
    use super::test_fakes::{TestJobService, TestLogService, TestMcpService, TestSkillsService};
    use dasclaw_app_server_protocol::McpServiceAvailability;

    #[test]
    fn app_server_mcp_capability_availability_requires_ready_health() {
        let services = AppServerServices::for_tests(
            TestLogService::ready(),
            TestJobService::ready(vec![]),
            TestSkillsService::ready(vec![]),
            TestMcpService::disabled_with_advertised_methods(vec![]),
        );

        assert_eq!(
            services.availability().mcp,
            McpServiceAvailability::default()
        );
    }
}
