use std::sync::{Arc, Mutex};

use dasclaw_app_server_protocol::{
    JobListParams, JobListResponse, JobReadParams, JobReadResponse, JobSnapshot, JobSnapshotState,
    ServiceHealth, ServiceName,
};
use dasclaw_core::error::JobError;
use dasclaw_runtime::JobState;
use dasclaw_runtime::context::{ContextManager, JobContext};
use uuid::Uuid;

use crate::AppServerError;
use crate::app_services::JobService;
use crate::blocking_runtime::BlockingTokioRuntime;

#[derive(Clone)]
pub struct AppServerJobService {
    manager: Arc<ContextManager>,
    runtime: Arc<Mutex<Option<BlockingTokioRuntime>>>,
}

impl AppServerJobService {
    pub fn new(manager: Arc<ContextManager>) -> Self {
        Self {
            manager,
            runtime: Arc::new(Mutex::new(None)),
        }
    }

    pub fn manager(&self) -> Arc<ContextManager> {
        Arc::clone(&self.manager)
    }

    fn runtime(&self) -> Result<BlockingTokioRuntime, AppServerError> {
        let mut runtime = self
            .runtime
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        if let Some(runtime) = runtime.as_ref() {
            return Ok(runtime.clone());
        }
        let created = BlockingTokioRuntime::new("dasclaw-app-server-jobs", "jobs")?;
        *runtime = Some(created.clone());
        Ok(created)
    }

    #[cfg(test)]
    fn worker_thread_id_for_tests(&self) -> Result<std::thread::ThreadId, AppServerError> {
        use std::thread;

        self.runtime()?
            .block_on("jobs/test-worker-id", async { Ok(thread::current().id()) })
    }
}

impl JobService for AppServerJobService {
    fn health(&self) -> ServiceHealth {
        ServiceHealth::ready(ServiceName::Jobs)
    }

    fn list(&self, params: JobListParams) -> Result<JobListResponse, AppServerError> {
        let manager = Arc::clone(&self.manager);
        self.runtime()?.block_on("jobs/list", async move {
            let mut jobs = Vec::new();
            for job_id in manager.all_jobs().await {
                let context = manager.get_context(job_id).await.map_err(map_job_error)?;
                jobs.push(job_snapshot_from_context(context));
            }
            jobs.sort_by(|left, right| {
                left.created_at
                    .cmp(&right.created_at)
                    .then_with(|| left.job_id.cmp(&right.job_id))
            });

            let start = parse_cursor(params.cursor.as_deref())?;
            let limit = parse_limit(params.limit, jobs.len())?;
            let end = start.saturating_add(limit).min(jobs.len());
            let data = jobs.get(start..end).unwrap_or(&[]).to_vec();
            let next_cursor = (end < jobs.len()).then(|| end.to_string());

            Ok(JobListResponse { data, next_cursor })
        })
    }

    fn read(&self, params: JobReadParams) -> Result<JobReadResponse, AppServerError> {
        let manager = Arc::clone(&self.manager);
        self.runtime()?.block_on("jobs/read", async move {
            let job_id = Uuid::parse_str(&params.job_id)
                .map_err(|_| AppServerError::invalid_request("jobs", "jobId must be a UUID"))?;
            let context = manager.get_context(job_id).await.map_err(map_job_error)?;
            Ok(JobReadResponse {
                job: job_snapshot_from_context(context),
            })
        })
    }
}

fn parse_cursor(cursor: Option<&str>) -> Result<usize, AppServerError> {
    match cursor {
        Some(cursor) => cursor.parse::<usize>().map_err(|_| {
            AppServerError::invalid_request("jobs", "cursor must be a non-negative integer")
        }),
        None => Ok(0),
    }
}

fn parse_limit(limit: Option<u32>, total_jobs: usize) -> Result<usize, AppServerError> {
    match limit {
        Some(0) => Err(AppServerError::invalid_request(
            "jobs",
            "limit must be greater than 0",
        )),
        Some(limit) => Ok(limit as usize),
        None => Ok(total_jobs.max(1)),
    }
}

fn job_snapshot_from_context(context: JobContext) -> JobSnapshot {
    let updated_at = latest_job_timestamp(&context);
    JobSnapshot {
        job_id: context.job_id.to_string(),
        title: context.title,
        description: context.description,
        state: map_job_state(context.state),
        created_at: context.created_at.to_rfc3339(),
        updated_at,
        thread_id: context.conversation_id.map(|id| id.to_string()),
    }
}

fn latest_job_timestamp(context: &JobContext) -> Option<String> {
    context
        .completed_at
        .or(context.started_at)
        .or_else(|| {
            context
                .transitions
                .last()
                .map(|transition| transition.timestamp)
        })
        .map(|timestamp| timestamp.to_rfc3339())
}

fn map_job_state(state: JobState) -> JobSnapshotState {
    match state {
        JobState::Pending => JobSnapshotState::Pending,
        JobState::InProgress => JobSnapshotState::InProgress,
        JobState::Completed => JobSnapshotState::Completed,
        JobState::Submitted => JobSnapshotState::Submitted,
        JobState::Accepted => JobSnapshotState::Accepted,
        JobState::Failed => JobSnapshotState::Failed,
        JobState::Stuck => JobSnapshotState::Stuck,
        JobState::Cancelled => JobSnapshotState::Cancelled,
    }
}

fn map_job_error(error: JobError) -> AppServerError {
    match error {
        JobError::NotFound { .. } => AppServerError::invalid_request("jobs", error.to_string()),
        _ => AppServerError::capability_unavailable("jobs", error.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use dasclaw_app_server_protocol::{JobListParams, JobReadParams, JobSnapshotState};
    use dasclaw_runtime::JobState;
    use dasclaw_runtime::context::ContextManager;

    use super::AppServerJobService;
    use crate::app_services::JobService;

    #[test]
    fn app_server_job_service_maps_runtime_states_without_downgrading() {
        let manager = Arc::new(ContextManager::new(20));
        let service = AppServerJobService::new(Arc::clone(&manager));
        let job_ids = run(async {
            let pending = manager.create_job("Pending", "waiting").await.unwrap();
            let in_progress = manager.create_job("Running", "active").await.unwrap();
            manager
                .update_context(in_progress, |ctx| {
                    ctx.transition_to(JobState::InProgress, None)
                })
                .await
                .unwrap()
                .unwrap();
            let completed = manager.create_job("Completed", "done").await.unwrap();
            manager
                .update_context(completed, |ctx| {
                    ctx.transition_to(JobState::InProgress, None).unwrap();
                    ctx.transition_to(JobState::Completed, None)
                })
                .await
                .unwrap()
                .unwrap();
            let submitted = manager.create_job("Submitted", "review").await.unwrap();
            manager
                .update_context(submitted, |ctx| {
                    ctx.transition_to(JobState::InProgress, None).unwrap();
                    ctx.transition_to(JobState::Completed, None).unwrap();
                    ctx.transition_to(JobState::Submitted, None)
                })
                .await
                .unwrap()
                .unwrap();
            let accepted = manager.create_job("Accepted", "paid").await.unwrap();
            manager
                .update_context(accepted, |ctx| {
                    ctx.transition_to(JobState::InProgress, None).unwrap();
                    ctx.transition_to(JobState::Completed, None).unwrap();
                    ctx.transition_to(JobState::Submitted, None).unwrap();
                    ctx.transition_to(JobState::Accepted, None)
                })
                .await
                .unwrap()
                .unwrap();
            let failed = manager.create_job("Failed", "broken").await.unwrap();
            manager
                .update_context(failed, |ctx| {
                    ctx.transition_to(JobState::InProgress, None).unwrap();
                    ctx.transition_to(JobState::Failed, None)
                })
                .await
                .unwrap()
                .unwrap();
            let stuck = manager.create_job("Stuck", "blocked").await.unwrap();
            manager
                .update_context(stuck, |ctx| {
                    ctx.transition_to(JobState::InProgress, None).unwrap();
                    ctx.transition_to(JobState::Stuck, None)
                })
                .await
                .unwrap()
                .unwrap();
            let cancelled = manager.create_job("Cancelled", "stopped").await.unwrap();
            manager
                .update_context(cancelled, |ctx| {
                    ctx.transition_to(JobState::Cancelled, None)
                })
                .await
                .unwrap()
                .unwrap();
            [
                pending,
                in_progress,
                completed,
                submitted,
                accepted,
                failed,
                stuck,
                cancelled,
            ]
        });

        let response = service.list(JobListParams::default()).unwrap();
        let states: Vec<_> = job_ids
            .iter()
            .map(|job_id| {
                response
                    .data
                    .iter()
                    .find(|job| job.job_id == job_id.to_string())
                    .expect("job should be listed")
                    .state
            })
            .collect();

        assert_eq!(
            states,
            vec![
                JobSnapshotState::Pending,
                JobSnapshotState::InProgress,
                JobSnapshotState::Completed,
                JobSnapshotState::Submitted,
                JobSnapshotState::Accepted,
                JobSnapshotState::Failed,
                JobSnapshotState::Stuck,
                JobSnapshotState::Cancelled,
            ]
        );
    }

    #[test]
    fn app_server_job_service_reads_single_runtime_context_by_uuid() {
        let manager = Arc::new(ContextManager::new(2));
        let service = AppServerJobService::new(Arc::clone(&manager));
        let job_id = run(async { manager.create_job("Inspect", "one job").await.unwrap() });

        let response = service
            .read(JobReadParams {
                job_id: job_id.to_string(),
            })
            .unwrap();

        assert_eq!(response.job.job_id, job_id.to_string());
        assert_eq!(response.job.title, "Inspect");
        assert_eq!(response.job.description, "one job");
        assert_eq!(response.job.state, JobSnapshotState::Pending);
    }

    #[test]
    fn app_server_job_service_reuses_one_worker_thread_for_requests() {
        let manager = Arc::new(ContextManager::new(2));
        let service = AppServerJobService::new(manager);

        let first = service.worker_thread_id_for_tests().unwrap();
        let second = service.worker_thread_id_for_tests().unwrap();

        assert_eq!(
            first, second,
            "job service must not create a new runtime thread per request"
        );
    }

    fn run<T>(future: impl std::future::Future<Output = T>) -> T {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(future)
    }
}
