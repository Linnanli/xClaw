use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;

use dasclaw_app_server_protocol::{
    AppServerP5Availability, AppServerR6Availability, AppServerServiceAvailability,
    CommandExecAvailability, CommandExecOutputDeltaNotification, CommandExecParams,
    CommandExecResizeParams, CommandExecResizeResponse, CommandExecResponse,
    CommandExecTerminateParams, CommandExecTerminateResponse, CommandExecWriteParams,
    CommandExecWriteResponse, ConfigBatchWriteParams, ConfigReadParams, ConfigReadResponse,
    ConfigValueWriteParams, ConfigWriteResponse, FsChangedNotification, FsCopyParams,
    FsCopyResponse, FsCreateDirectoryParams, FsCreateDirectoryResponse, FsGetMetadataParams,
    FsGetMetadataResponse, FsReadDirectoryParams, FsReadDirectoryResponse, FsReadFileParams,
    FsReadFileResponse, FsRemoveParams, FsRemoveResponse, FsUnwatchParams, FsUnwatchResponse,
    FsWatchParams, FsWatchResponse, FsWriteFileParams, FsWriteFileResponse, FuzzyFileSearchParams,
    FuzzyFileSearchResponse, GitDiffToRemoteParams, GitDiffToRemoteResponse, JobListParams,
    JobListResponse, JobReadParams, JobReadResponse, ListMcpServerStatusParams,
    ListMcpServerStatusResponse, LogEntryEvent, McpResourceReadParams, McpResourceReadResponse,
    McpServerOauthLoginParams, McpServerOauthLoginResponse, McpServerReloadParams,
    McpServerReloadResponse, McpServerToolCallParams, McpServerToolCallResponse,
    McpServiceAvailability, McpToolCallProgressNotification, ServiceHealth, ServiceName,
    ServiceStatus, SkillsConfigWriteParams, SkillsConfigWriteResponse, SkillsListParams,
    SkillsListResponse,
};
use dasclaw_hooks::{HookRegistry, HookRunObserver};
use dasclaw_runtime::context::ContextManager;

use crate::AppServerError;
use crate::command_service::AppServerCommandExecService;
use crate::config_service::AppServerConfigService;
use crate::fs_service::AppServerFsService;
use crate::hook_service::{AppServerHookNotification, AppServerHookService};
use crate::job_service::AppServerJobService;
use crate::log_service::AppServerLogService;
use crate::mcp_service::AppServerMcpService;
use crate::repo_service::AppServerRepoService;
use crate::search_service::{AppServerSearchService, SearchNotification};
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

pub trait FsService: Send + Sync {
    fn health(&self) -> ServiceHealth;
    fn read_file(&self, params: FsReadFileParams) -> Result<FsReadFileResponse, AppServerError>;
    fn write_file(&self, params: FsWriteFileParams) -> Result<FsWriteFileResponse, AppServerError>;
    fn create_directory(
        &self,
        params: FsCreateDirectoryParams,
    ) -> Result<FsCreateDirectoryResponse, AppServerError>;
    fn get_metadata(
        &self,
        params: FsGetMetadataParams,
    ) -> Result<FsGetMetadataResponse, AppServerError>;
    fn read_directory(
        &self,
        params: FsReadDirectoryParams,
    ) -> Result<FsReadDirectoryResponse, AppServerError>;
    fn remove(&self, params: FsRemoveParams) -> Result<FsRemoveResponse, AppServerError>;
    fn copy(&self, params: FsCopyParams) -> Result<FsCopyResponse, AppServerError>;
    fn watch(&self, params: FsWatchParams) -> Result<FsWatchResponse, AppServerError>;
    fn unwatch(&self, params: FsUnwatchParams) -> Result<FsUnwatchResponse, AppServerError>;

    fn drain_changed_events(&self) -> Vec<FsChangedNotification> {
        Vec::new()
    }

    fn is_ready(&self) -> bool {
        self.health().status == ServiceStatus::Ready
    }
}

pub trait CommandExecService: Send + Sync {
    fn health(&self) -> ServiceHealth;
    fn exec(&self, params: CommandExecParams) -> Result<CommandExecResponse, AppServerError>;
    fn write(
        &self,
        params: CommandExecWriteParams,
    ) -> Result<CommandExecWriteResponse, AppServerError>;
    fn terminate(
        &self,
        params: CommandExecTerminateParams,
    ) -> Result<CommandExecTerminateResponse, AppServerError>;
    fn resize(
        &self,
        params: CommandExecResizeParams,
    ) -> Result<CommandExecResizeResponse, AppServerError>;

    fn drain_output_delta_events(&self) -> Vec<CommandExecOutputDeltaNotification> {
        Vec::new()
    }

    fn has_active_process(&self, _process_id: &str) -> bool {
        false
    }

    fn availability(&self) -> CommandExecAvailability {
        CommandExecAvailability::default()
    }

    fn is_ready(&self) -> bool {
        self.health().status == ServiceStatus::Ready
    }
}

pub trait ConfigService: Send + Sync {
    fn health(&self) -> ServiceHealth;
    fn read(&self, params: ConfigReadParams) -> Result<ConfigReadResponse, AppServerError>;
    fn write_value(
        &self,
        params: ConfigValueWriteParams,
    ) -> Result<ConfigWriteResponse, AppServerError>;
    fn write_batch(
        &self,
        params: ConfigBatchWriteParams,
    ) -> Result<ConfigWriteResponse, AppServerError>;

    fn collect_startup_notifications(&self) -> Vec<AppServerHookNotification> {
        Vec::new()
    }

    fn drain_config_notifications(&self) -> Vec<AppServerHookNotification> {
        Vec::new()
    }

    fn is_ready(&self) -> bool {
        self.health().status == ServiceStatus::Ready
    }
}

pub trait RepoService: Send + Sync {
    fn health(&self) -> ServiceHealth;
    fn git_diff_to_remote(
        &self,
        params: GitDiffToRemoteParams,
    ) -> Result<GitDiffToRemoteResponse, AppServerError>;

    fn is_ready(&self) -> bool {
        self.health().status == ServiceStatus::Ready
    }
}

pub trait SearchService: Send + Sync {
    fn health(&self) -> ServiceHealth;
    fn fuzzy_file_search(
        &self,
        params: FuzzyFileSearchParams,
    ) -> Result<FuzzyFileSearchResponse, AppServerError>;

    fn drain_search_events(&self) -> Vec<SearchNotification> {
        Vec::new()
    }

    fn is_ready(&self) -> bool {
        self.health().status == ServiceStatus::Ready
    }
}

pub trait HookNotificationService: Send + Sync {
    fn health(&self) -> ServiceHealth;
    fn drain_hook_notifications(&self) -> Vec<AppServerHookNotification>;
    fn push_hook_notifications(&self, _pending: Vec<AppServerHookNotification>) {}

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
    pub filesystem: Arc<dyn FsService>,
    pub command: Arc<dyn CommandExecService>,
    pub config: Arc<dyn ConfigService>,
    pub repo: Arc<dyn RepoService>,
    pub search: Arc<dyn SearchService>,
    pub hooks: Arc<dyn HookNotificationService>,
    pub hook_registry: Option<Arc<HookRegistry>>,
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
            filesystem: Arc::new(NoopFsService),
            command: Arc::new(NoopCommandExecService),
            config: Arc::new(NoopConfigService),
            repo: Arc::new(NoopRepoService),
            search: Arc::new(NoopSearchService),
            hooks: Arc::new(NoopHookService),
            hook_registry: None,
        }
    }
}

impl AppServerServices {
    #[must_use]
    pub fn real() -> Self {
        let root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        Self::real_with_root(root)
    }

    fn real_with_root(root: PathBuf) -> Self {
        let manager = Arc::new(ContextManager::default());
        let hook_service = Arc::new(AppServerHookService::wired());
        let hook_observer: Arc<dyn HookRunObserver> = hook_service.clone();
        let hook_registry = Arc::new(HookRegistry::new().with_observer(hook_observer));
        Self {
            logs: Arc::new(AppServerLogService::new()),
            jobs: Arc::new(AppServerJobService::new(manager)),
            skills: Arc::new(AppServerSkillsService::new()),
            mcp: Arc::new(AppServerMcpService::default()),
            filesystem: Arc::new(AppServerFsService::new(root.clone())),
            command: Arc::new(AppServerCommandExecService::new(root.clone())),
            config: Arc::new(AppServerConfigService::new(root.clone())),
            repo: Arc::new(AppServerRepoService::new(root.clone())),
            search: Arc::new(AppServerSearchService::new(root)),
            hooks: hook_service,
            hook_registry: Some(hook_registry),
        }
    }

    pub fn health(&self) -> Vec<ServiceHealth> {
        vec![
            self.logs.health(),
            self.jobs.health(),
            self.skills.health(),
            self.mcp.health(),
            self.filesystem.health(),
            self.command.health(),
            self.config.health(),
            self.repo.health(),
            self.search.health(),
            self.hooks.health(),
        ]
    }

    pub fn drain_log_entries(&self) -> Vec<LogEntryEvent> {
        self.logs.drain_entries()
    }

    pub fn drain_mcp_tool_call_progress_events(&self) -> Vec<McpToolCallProgressNotification> {
        self.mcp.drain_tool_call_progress_events()
    }

    pub fn drain_fs_changed_events(&self) -> Vec<FsChangedNotification> {
        self.filesystem.drain_changed_events()
    }

    pub fn drain_command_exec_output_delta_events(
        &self,
    ) -> Vec<CommandExecOutputDeltaNotification> {
        self.command.drain_output_delta_events()
    }

    pub fn drain_search_events(&self) -> Vec<SearchNotification> {
        self.search.drain_search_events()
    }

    pub fn drain_hook_notifications(&self) -> Vec<AppServerHookNotification> {
        let mut notifications = self.hooks.drain_hook_notifications();
        notifications.extend(self.config.drain_config_notifications());
        notifications
    }

    pub fn collect_startup_notifications(&self) {
        let mut pending = self.config.collect_startup_notifications();
        pending.extend(self.config.drain_config_notifications());
        if !pending.is_empty() {
            self.hooks.push_hook_notifications(pending);
        }
    }

    pub fn availability(&self) -> AppServerServiceAvailability {
        let command = if self.command.is_ready() {
            self.command.availability()
        } else {
            CommandExecAvailability::default()
        };

        let hook_notifications_ready = self.hooks.is_ready();
        let config_ready = self.config.is_ready();

        AppServerServiceAvailability {
            logs: self.logs.is_ready(),
            jobs: self.jobs.is_ready(),
            skills: self.skills.is_ready(),
            mcp: if self.mcp.is_ready() {
                self.mcp.availability()
            } else {
                McpServiceAvailability::default()
            },
            p5: AppServerP5Availability {
                filesystem: self.filesystem.is_ready(),
                command,
            },
            r6: AppServerR6Availability {
                config: config_ready,
                repo: self.repo.is_ready(),
                search: self.search.is_ready(),
                hooks: hook_notifications_ready,
                warnings: false,
                config_warnings: config_ready && hook_notifications_ready,
                deprecation_notices: config_ready && hook_notifications_ready,
                guardian_warnings: false,
                ..AppServerR6Availability::default()
            },
        }
    }

    #[cfg(test)]
    pub fn real_with_root_for_tests(root: PathBuf) -> Self {
        Self::real_with_root(root)
    }

    #[cfg(test)]
    pub fn for_tests(
        logs: impl LogService + 'static,
        jobs: impl JobService + 'static,
        skills: impl SkillsService + 'static,
        mcp: impl McpService + 'static,
        filesystem: impl FsService + 'static,
        command: impl CommandExecService + 'static,
    ) -> Self {
        Self {
            logs: Arc::new(logs),
            jobs: Arc::new(jobs),
            skills: Arc::new(skills),
            mcp: Arc::new(mcp),
            filesystem: Arc::new(filesystem),
            command: Arc::new(command),
            config: Arc::new(NoopConfigService),
            repo: Arc::new(NoopRepoService),
            search: Arc::new(NoopSearchService),
            hooks: Arc::new(NoopHookService),
            hook_registry: None,
        }
    }
}

struct NoopLogService;
struct NoopJobService;
struct NoopSkillsService;
struct NoopMcpService;
struct NoopFsService;
struct NoopCommandExecService;
struct NoopConfigService;
struct NoopRepoService;
struct NoopSearchService;
struct NoopHookService;

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

impl FsService for NoopFsService {
    fn health(&self) -> ServiceHealth {
        ServiceHealth::disabled(ServiceName::Filesystem, "filesystem service is not wired")
    }

    fn read_file(&self, _params: FsReadFileParams) -> Result<FsReadFileResponse, AppServerError> {
        filesystem_not_wired()
    }

    fn write_file(
        &self,
        _params: FsWriteFileParams,
    ) -> Result<FsWriteFileResponse, AppServerError> {
        filesystem_not_wired()
    }

    fn create_directory(
        &self,
        _params: FsCreateDirectoryParams,
    ) -> Result<FsCreateDirectoryResponse, AppServerError> {
        filesystem_not_wired()
    }

    fn get_metadata(
        &self,
        _params: FsGetMetadataParams,
    ) -> Result<FsGetMetadataResponse, AppServerError> {
        filesystem_not_wired()
    }

    fn read_directory(
        &self,
        _params: FsReadDirectoryParams,
    ) -> Result<FsReadDirectoryResponse, AppServerError> {
        filesystem_not_wired()
    }

    fn remove(&self, _params: FsRemoveParams) -> Result<FsRemoveResponse, AppServerError> {
        filesystem_not_wired()
    }

    fn copy(&self, _params: FsCopyParams) -> Result<FsCopyResponse, AppServerError> {
        filesystem_not_wired()
    }

    fn watch(&self, _params: FsWatchParams) -> Result<FsWatchResponse, AppServerError> {
        filesystem_not_wired()
    }

    fn unwatch(&self, _params: FsUnwatchParams) -> Result<FsUnwatchResponse, AppServerError> {
        filesystem_not_wired()
    }
}

impl CommandExecService for NoopCommandExecService {
    fn health(&self) -> ServiceHealth {
        ServiceHealth::disabled(
            ServiceName::CommandExec,
            "command execution service is not wired",
        )
    }

    fn exec(&self, _params: CommandExecParams) -> Result<CommandExecResponse, AppServerError> {
        command_exec_not_wired()
    }

    fn write(
        &self,
        _params: CommandExecWriteParams,
    ) -> Result<CommandExecWriteResponse, AppServerError> {
        command_exec_not_wired()
    }

    fn terminate(
        &self,
        _params: CommandExecTerminateParams,
    ) -> Result<CommandExecTerminateResponse, AppServerError> {
        command_exec_not_wired()
    }

    fn resize(
        &self,
        _params: CommandExecResizeParams,
    ) -> Result<CommandExecResizeResponse, AppServerError> {
        command_exec_not_wired()
    }
}

fn filesystem_not_wired<T>() -> Result<T, AppServerError> {
    Err(AppServerError::capability_unavailable(
        "filesystem",
        "filesystem service is not wired",
    ))
}

fn command_exec_not_wired<T>() -> Result<T, AppServerError> {
    Err(AppServerError::capability_unavailable(
        "command_exec",
        "command execution service is not wired",
    ))
}

impl ConfigService for NoopConfigService {
    fn health(&self) -> ServiceHealth {
        ServiceHealth::disabled(ServiceName::Config, "config service is not wired")
    }

    fn read(&self, _params: ConfigReadParams) -> Result<ConfigReadResponse, AppServerError> {
        config_not_wired()
    }

    fn write_value(
        &self,
        _params: ConfigValueWriteParams,
    ) -> Result<ConfigWriteResponse, AppServerError> {
        config_not_wired()
    }

    fn write_batch(
        &self,
        _params: ConfigBatchWriteParams,
    ) -> Result<ConfigWriteResponse, AppServerError> {
        config_not_wired()
    }
}

fn config_not_wired<T>() -> Result<T, AppServerError> {
    Err(AppServerError::capability_unavailable(
        "config",
        "config service is not wired",
    ))
}

impl RepoService for NoopRepoService {
    fn health(&self) -> ServiceHealth {
        ServiceHealth::disabled(ServiceName::Repo, "repo service is not wired")
    }

    fn git_diff_to_remote(
        &self,
        _params: GitDiffToRemoteParams,
    ) -> Result<GitDiffToRemoteResponse, AppServerError> {
        Err(AppServerError::capability_unavailable(
            "repo",
            "repo service is not wired",
        ))
    }
}

impl SearchService for NoopSearchService {
    fn health(&self) -> ServiceHealth {
        ServiceHealth::disabled(ServiceName::Search, "search service is not wired")
    }

    fn fuzzy_file_search(
        &self,
        _params: FuzzyFileSearchParams,
    ) -> Result<FuzzyFileSearchResponse, AppServerError> {
        Err(AppServerError::capability_unavailable(
            "search",
            "search service is not wired",
        ))
    }
}

impl HookNotificationService for NoopHookService {
    fn health(&self) -> ServiceHealth {
        ServiceHealth::disabled(ServiceName::Hooks, "hook notification service is not wired")
    }

    fn drain_hook_notifications(&self) -> Vec<AppServerHookNotification> {
        Vec::new()
    }

    fn push_hook_notifications(&self, _pending: Vec<AppServerHookNotification>) {}
}

#[cfg(test)]
pub use test_fakes::{
    TestCommandExecService, TestFsService, TestJobService, TestLogService, TestMcpService,
    TestSkillsService,
};

#[cfg(test)]
mod test_fakes {
    use std::sync::{Arc, Mutex};

    use dasclaw_app_server_protocol::{
        CommandExecAvailability, CommandExecOutputDeltaNotification, CommandExecParams,
        CommandExecResizeParams, CommandExecResizeResponse, CommandExecResponse,
        CommandExecTerminateParams, CommandExecTerminateResponse, CommandExecWriteParams,
        CommandExecWriteResponse, FsChangedNotification, FsCopyParams, FsCopyResponse,
        FsCreateDirectoryParams, FsCreateDirectoryResponse, FsGetMetadataParams,
        FsGetMetadataResponse, FsReadDirectoryParams, FsReadDirectoryResponse, FsReadFileParams,
        FsReadFileResponse, FsRemoveParams, FsRemoveResponse, FsUnwatchParams, FsUnwatchResponse,
        FsWatchParams, FsWatchResponse, FsWriteFileParams, FsWriteFileResponse, JobListParams,
        JobListResponse, JobReadParams, JobReadResponse, JobSnapshot, ListMcpServerStatusParams,
        ListMcpServerStatusResponse, LogEntryEvent, McpResourceReadParams, McpResourceReadResponse,
        McpServerOauthLoginParams, McpServerOauthLoginResponse, McpServerReloadParams,
        McpServerReloadResponse, McpServerStatus, McpServerToolCallParams,
        McpServerToolCallResponse, McpServiceAvailability, ServiceHealth, ServiceName,
        SkillsConfigWriteParams, SkillsConfigWriteResponse, SkillsListEntry, SkillsListParams,
        SkillsListResponse,
    };

    use super::{CommandExecService, FsService, JobService, LogService, McpService, SkillsService};
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

    #[derive(Clone)]
    pub struct TestFsService {
        ready: bool,
        changed_events: Arc<Mutex<Vec<FsChangedNotification>>>,
    }

    impl TestFsService {
        pub fn ready() -> Self {
            Self {
                ready: true,
                changed_events: Arc::new(Mutex::new(Vec::new())),
            }
        }

        pub fn disabled() -> Self {
            Self {
                ready: false,
                changed_events: Arc::new(Mutex::new(Vec::new())),
            }
        }

        pub fn ready_with_changed_events(events: Vec<FsChangedNotification>) -> Self {
            Self {
                ready: true,
                changed_events: Arc::new(Mutex::new(events)),
            }
        }

        fn require_ready(&self) -> Result<(), AppServerError> {
            if self.ready {
                Ok(())
            } else {
                test_fs_disabled()
            }
        }
    }

    impl FsService for TestFsService {
        fn health(&self) -> ServiceHealth {
            if self.ready {
                ServiceHealth::ready(ServiceName::Filesystem)
            } else {
                ServiceHealth::disabled(ServiceName::Filesystem, "test filesystem service disabled")
            }
        }

        fn read_file(
            &self,
            _params: FsReadFileParams,
        ) -> Result<FsReadFileResponse, AppServerError> {
            self.require_ready()?;
            Ok(FsReadFileResponse {
                data_base64: "dGVzdA==".to_string(),
            })
        }

        fn write_file(
            &self,
            _params: FsWriteFileParams,
        ) -> Result<FsWriteFileResponse, AppServerError> {
            self.require_ready()?;
            Ok(FsWriteFileResponse {})
        }

        fn create_directory(
            &self,
            _params: FsCreateDirectoryParams,
        ) -> Result<FsCreateDirectoryResponse, AppServerError> {
            self.require_ready()?;
            Ok(FsCreateDirectoryResponse {})
        }

        fn get_metadata(
            &self,
            _params: FsGetMetadataParams,
        ) -> Result<FsGetMetadataResponse, AppServerError> {
            self.require_ready()?;
            Ok(FsGetMetadataResponse {
                is_file: true,
                is_directory: false,
                is_symlink: false,
                created_at_ms: 0,
                modified_at_ms: 0,
            })
        }

        fn read_directory(
            &self,
            _params: FsReadDirectoryParams,
        ) -> Result<FsReadDirectoryResponse, AppServerError> {
            self.require_ready()?;
            Ok(FsReadDirectoryResponse { entries: vec![] })
        }

        fn remove(&self, _params: FsRemoveParams) -> Result<FsRemoveResponse, AppServerError> {
            self.require_ready()?;
            Ok(FsRemoveResponse {})
        }

        fn copy(&self, _params: FsCopyParams) -> Result<FsCopyResponse, AppServerError> {
            self.require_ready()?;
            Ok(FsCopyResponse {})
        }

        fn watch(&self, params: FsWatchParams) -> Result<FsWatchResponse, AppServerError> {
            self.require_ready()?;
            Ok(FsWatchResponse { path: params.path })
        }

        fn unwatch(&self, _params: FsUnwatchParams) -> Result<FsUnwatchResponse, AppServerError> {
            self.require_ready()?;
            Ok(FsUnwatchResponse {})
        }

        fn drain_changed_events(&self) -> Vec<FsChangedNotification> {
            self.changed_events
                .lock()
                .map(|mut events| std::mem::take(&mut *events))
                .unwrap_or_default()
        }
    }

    #[derive(Clone)]
    pub struct TestCommandExecService {
        availability: CommandExecAvailability,
        output_delta_events: Arc<Mutex<Vec<CommandExecOutputDeltaNotification>>>,
    }

    impl TestCommandExecService {
        pub fn ready_buffered() -> Self {
            Self {
                availability: CommandExecAvailability {
                    exec: true,
                    ..CommandExecAvailability::default()
                },
                output_delta_events: Arc::new(Mutex::new(Vec::new())),
            }
        }

        pub fn ready_streaming() -> Self {
            Self {
                availability: CommandExecAvailability {
                    exec: true,
                    output_delta_events: true,
                    terminate: true,
                    write: true,
                    resize: true,
                },
                output_delta_events: Arc::new(Mutex::new(Vec::new())),
            }
        }

        pub fn disabled() -> Self {
            Self {
                availability: CommandExecAvailability::default(),
                output_delta_events: Arc::new(Mutex::new(Vec::new())),
            }
        }

        pub fn ready_with_output_delta_events(
            events: Vec<CommandExecOutputDeltaNotification>,
        ) -> Self {
            Self {
                availability: CommandExecAvailability {
                    exec: true,
                    output_delta_events: true,
                    ..CommandExecAvailability::default()
                },
                output_delta_events: Arc::new(Mutex::new(events)),
            }
        }

        fn require_ready(&self) -> Result<(), AppServerError> {
            if self.availability != CommandExecAvailability::default() {
                Ok(())
            } else {
                test_command_exec_disabled()
            }
        }
    }

    impl CommandExecService for TestCommandExecService {
        fn health(&self) -> ServiceHealth {
            if self.availability != CommandExecAvailability::default() {
                ServiceHealth::ready(ServiceName::CommandExec)
            } else {
                ServiceHealth::disabled(
                    ServiceName::CommandExec,
                    "test command execution service disabled",
                )
            }
        }

        fn exec(&self, _params: CommandExecParams) -> Result<CommandExecResponse, AppServerError> {
            self.require_ready()?;
            Ok(CommandExecResponse {
                exit_code: 0,
                stdout: "test".to_string(),
                stderr: String::new(),
            })
        }

        fn write(
            &self,
            _params: CommandExecWriteParams,
        ) -> Result<CommandExecWriteResponse, AppServerError> {
            self.require_ready()?;
            Err(AppServerError::capability_unavailable(
                "command_exec",
                "command stdin streaming is not available in this test service",
            ))
        }

        fn terminate(
            &self,
            _params: CommandExecTerminateParams,
        ) -> Result<CommandExecTerminateResponse, AppServerError> {
            self.require_ready()?;
            Err(AppServerError::capability_unavailable(
                "command_exec",
                "command termination is not available in this test service",
            ))
        }

        fn resize(
            &self,
            _params: CommandExecResizeParams,
        ) -> Result<CommandExecResizeResponse, AppServerError> {
            self.require_ready()?;
            Err(AppServerError::capability_unavailable(
                "command_exec",
                "command resize is not available in this test service",
            ))
        }

        fn availability(&self) -> CommandExecAvailability {
            self.availability
        }

        fn drain_output_delta_events(&self) -> Vec<CommandExecOutputDeltaNotification> {
            self.output_delta_events
                .lock()
                .map(|mut events| std::mem::take(&mut *events))
                .unwrap_or_default()
        }
    }

    fn test_fs_disabled<T>() -> Result<T, AppServerError> {
        Err(AppServerError::capability_unavailable(
            "filesystem",
            "test filesystem disabled",
        ))
    }

    fn test_command_exec_disabled<T>() -> Result<T, AppServerError> {
        Err(AppServerError::capability_unavailable(
            "command_exec",
            "test command exec disabled",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::AppServerServices;
    use super::test_fakes::{
        TestCommandExecService, TestFsService, TestJobService, TestLogService, TestMcpService,
        TestSkillsService,
    };
    use dasclaw_app_server_protocol::McpServiceAvailability;

    #[test]
    fn app_server_mcp_capability_availability_requires_ready_health() {
        let services = AppServerServices::for_tests(
            TestLogService::ready(),
            TestJobService::ready(vec![]),
            TestSkillsService::ready(vec![]),
            TestMcpService::disabled_with_advertised_methods(vec![]),
            TestFsService::disabled(),
            TestCommandExecService::disabled(),
        );

        assert_eq!(
            services.availability().mcp,
            McpServiceAvailability::default()
        );
    }

    #[test]
    fn app_server_p5_availability_requires_ready_health() {
        let services = AppServerServices::for_tests(
            TestLogService::ready(),
            TestJobService::ready(vec![]),
            TestSkillsService::ready(vec![]),
            TestMcpService::ready(vec![]),
            TestFsService::disabled(),
            TestCommandExecService::disabled(),
        );

        let availability = services.availability();
        assert!(!availability.p5.filesystem);
        assert!(!availability.p5.command.exec);
    }

    #[test]
    fn app_server_p5_availability_reports_ready_service_methods() {
        let services = AppServerServices::for_tests(
            TestLogService::ready(),
            TestJobService::ready(vec![]),
            TestSkillsService::ready(vec![]),
            TestMcpService::ready(vec![]),
            TestFsService::ready(),
            TestCommandExecService::ready_buffered(),
        );

        let availability = services.availability();
        assert!(availability.p5.filesystem);
        assert!(availability.p5.command.exec);
        assert!(!availability.p5.command.output_delta_events);
        assert!(!availability.p5.command.write);
        assert!(!availability.p5.command.resize);
    }

    #[test]
    fn app_server_real_wires_filesystem_and_command_exec() {
        let services = AppServerServices::real();

        let availability = services.availability();
        assert!(availability.p5.filesystem);
        assert!(availability.p5.command.exec);
        assert!(availability.p5.command.output_delta_events);
        assert!(availability.p5.command.write);
        assert!(availability.p5.command.terminate);
        assert!(availability.p5.command.resize);
    }
}
