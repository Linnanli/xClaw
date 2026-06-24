use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use dasclaw_app_server_protocol::{
    CodexGitInfo, CodexThreadItem, SandboxMode, ThreadGoal, ThreadGoalStatus, ThreadTokenUsage,
    TokenUsageBreakdown, TurnStatus,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{AppServerError, RuntimeSandboxContext, RuntimeTurnOutcome, RuntimeTurnUpdate};

const SNAPSHOT_VERSION: u32 = 1;

#[derive(Debug, Clone, Default)]
pub struct ThreadLifecycleHost {
    next_thread_id: u64,
    next_turn_id: u64,
    threads: Vec<ThreadRecord>,
    turns: Vec<TurnRecord>,
    snapshot_path: Option<PathBuf>,
}

impl ThreadLifecycleHost {
    #[must_use]
    pub fn new() -> Self {
        Self {
            next_thread_id: 1,
            next_turn_id: 1,
            threads: Vec::new(),
            turns: Vec::new(),
            snapshot_path: None,
        }
    }

    pub fn with_snapshot_path(path: impl Into<PathBuf>) -> Result<Self, AppServerError> {
        let path = path.into();
        if !path.exists() {
            return Ok(Self {
                snapshot_path: Some(path),
                ..Self::new()
            });
        }

        let content = fs::read_to_string(&path).map_err(|error| {
            thread_lifecycle_error(format!("failed to read thread lifecycle snapshot: {error}"))
        })?;
        if content.trim().is_empty() {
            return Ok(Self {
                snapshot_path: Some(path),
                ..Self::new()
            });
        }

        let snapshot: ThreadLifecycleSnapshot =
            serde_json::from_str(&content).map_err(|error| {
                thread_lifecycle_error(format!("invalid thread lifecycle snapshot JSON: {error}"))
            })?;
        let threads = snapshot
            .threads
            .into_iter()
            .map(ThreadRecord::from_persisted)
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Self {
            next_thread_id: snapshot.next_thread_id.max(1),
            next_turn_id: snapshot.next_turn_id.max(1),
            threads,
            turns: snapshot.turns.into_iter().map(Into::into).collect(),
            snapshot_path: Some(path),
        })
    }

    pub fn create(&mut self, params: ThreadCreation) -> Result<String, AppServerError> {
        let cwd = cwd_path(params.cwd.as_deref());
        let sandbox_context = crate::sandbox_protocol::resolve_thread_context(
            params.sandbox,
            params.permission_profile.clone(),
            cwd,
            "thread/start",
        )?;
        let mut next = self.clone();
        let thread_id = format!("thread_{}", next.next_thread_id);
        next.next_thread_id += 1;
        let now = unix_timestamp();
        next.threads.push(ThreadRecord {
            thread_id: thread_id.clone(),
            title: params.title,
            workspace_root: params.cwd,
            sandbox_context,
            sandbox: params.sandbox,
            permission_profile: params.permission_profile,
            forked_from_id: params.forked_from_id,
            ephemeral: params.ephemeral,
            archived: false,
            subscribed: false,
            path: None,
            created_at: now,
            updated_at: now,
            git_info: None,
            goal: None,
            compacted_turn_id: None,
            token_usage: None,
        });
        self.commit(next)?;
        Ok(thread_id)
    }

    pub fn list(&self) -> Vec<ThreadSummary> {
        self.threads.iter().map(ThreadRecord::to_summary).collect()
    }

    pub fn summary(&self, thread_id: &str) -> Option<ThreadSummary> {
        self.threads
            .iter()
            .find(|thread| thread.thread_id == thread_id)
            .map(ThreadRecord::to_summary)
    }

    #[must_use]
    pub fn next_turn_id(&self) -> String {
        format!("turn_{}", self.next_turn_id)
    }

    pub fn record_started_turn(
        &mut self,
        thread_id: &str,
        turn_id: String,
    ) -> Result<(), AppServerError> {
        self.ensure_thread(thread_id)?;
        let mut next = self.clone();
        next.next_turn_id += 1;
        next.turns.push(TurnRecord {
            thread_id: thread_id.to_string(),
            turn_id,
            status: TurnStatus::Pending,
            output: None,
            items: Vec::new(),
            error: None,
        });
        next.touch_thread(thread_id)?;
        self.commit(next)
    }

    pub fn apply_runtime_turn_update(
        &mut self,
        update: RuntimeTurnUpdate,
    ) -> Result<Option<TurnSummary>, AppServerError> {
        let (output, error) = match update.outcome {
            RuntimeTurnOutcome::Delta { .. }
            | RuntimeTurnOutcome::ReasoningSummaryDelta { .. }
            | RuntimeTurnOutcome::ReasoningSummaryPartAdded { .. }
            | RuntimeTurnOutcome::ReasoningTextDelta { .. }
            | RuntimeTurnOutcome::PlanUpdated { .. }
            | RuntimeTurnOutcome::DiffUpdated { .. }
            | RuntimeTurnOutcome::RawResponseItemCompleted { .. }
            | RuntimeTurnOutcome::PlanDelta { .. }
            | RuntimeTurnOutcome::ApprovalRequested { .. }
            | RuntimeTurnOutcome::ToolResult { .. }
            | RuntimeTurnOutcome::CommandOutputDelta { .. }
            | RuntimeTurnOutcome::DynamicToolCallRequested { .. }
            | RuntimeTurnOutcome::ToolUserInputRequested { .. }
            | RuntimeTurnOutcome::FileChangeApprovalRequested { .. }
            | RuntimeTurnOutcome::PermissionsApprovalRequested { .. }
            | RuntimeTurnOutcome::FileChangeOutputDelta { .. }
            | RuntimeTurnOutcome::FileChangePatchUpdated { .. }
            | RuntimeTurnOutcome::AutoApprovalReviewStarted { .. }
            | RuntimeTurnOutcome::AutoApprovalReviewCompleted { .. }
            | RuntimeTurnOutcome::TokenUsageUpdated { .. } => return Ok(None),
            RuntimeTurnOutcome::Completed { output } => (Some(output), None),
            RuntimeTurnOutcome::Failed { error } => (None, Some(error)),
        };

        let mut next = self.clone();
        let Some(turn) = next
            .turns
            .iter_mut()
            .find(|turn| turn.thread_id == update.thread_id && turn.turn_id == update.turn_id)
        else {
            return Ok(None);
        };
        if turn.status != TurnStatus::Pending {
            return Ok(None);
        }

        turn.status = if error.is_some() {
            TurnStatus::Failed
        } else {
            TurnStatus::Completed
        };
        turn.output = output;
        turn.error = error;

        let summary = turn.to_summary();
        next.touch_thread(&summary.thread_id)?;
        self.commit(next)?;
        Ok(Some(summary))
    }

    #[must_use]
    pub fn turn_is_pending(&self, thread_id: &str, turn_id: &str) -> bool {
        self.turns.iter().any(|turn| {
            turn.thread_id == thread_id
                && turn.turn_id == turn_id
                && turn.status == TurnStatus::Pending
        })
    }

    #[must_use]
    pub fn has_pending_turns(&self) -> bool {
        self.turns
            .iter()
            .any(|turn| turn.status == TurnStatus::Pending)
    }

    pub fn cancel_turn(
        &mut self,
        thread_id: &str,
        turn_id: &str,
    ) -> Result<Option<bool>, AppServerError> {
        let mut next = self.clone();
        let Some(turn) = next
            .turns
            .iter_mut()
            .find(|turn| turn.thread_id == thread_id && turn.turn_id == turn_id)
        else {
            return Ok(None);
        };
        if turn.status == TurnStatus::Cancelled {
            return Ok(Some(false));
        }

        turn.status = TurnStatus::Cancelled;
        next.touch_thread(thread_id)?;
        self.commit(next)?;
        Ok(Some(true))
    }

    pub fn cancel_pending_turns(&mut self) -> Result<Vec<TurnSummary>, AppServerError> {
        let mut next = self.clone();
        let mut cancelled = Vec::new();
        for turn in &mut next.turns {
            if turn.status == TurnStatus::Pending {
                turn.status = TurnStatus::Cancelled;
                turn.output = None;
                turn.error = None;
                cancelled.push(turn.to_summary());
            }
        }
        for turn in &cancelled {
            next.touch_thread(&turn.thread_id)?;
        }
        if !cancelled.is_empty() {
            self.commit(next)?;
        }
        Ok(cancelled)
    }

    #[must_use]
    pub fn list_turns(&self, thread_id: &str) -> Vec<TurnSummary> {
        self.turns
            .iter()
            .filter(|turn| turn.thread_id == thread_id)
            .map(TurnRecord::to_summary)
            .collect()
    }

    #[must_use]
    pub fn turn_summary(&self, thread_id: &str, turn_id: &str) -> Option<TurnSummary> {
        self.turns
            .iter()
            .find(|turn| turn.thread_id == thread_id && turn.turn_id == turn_id)
            .map(TurnRecord::to_summary)
    }

    pub fn rollback(&mut self, thread_id: &str, count: usize) -> Result<usize, AppServerError> {
        self.ensure_no_pending_turns(thread_id)?;
        let mut next = self.clone();
        let target_indexes = next
            .turns
            .iter()
            .enumerate()
            .rev()
            .filter_map(|(index, turn)| (turn.thread_id == thread_id).then_some(index))
            .take(count)
            .collect::<Vec<_>>();

        let removed = target_indexes.len();
        for index in target_indexes {
            next.turns.remove(index);
        }
        if removed > 0 {
            next.touch_thread(thread_id)?;
            self.commit(next)?;
        }
        Ok(removed)
    }

    pub fn set_name(
        &mut self,
        thread_id: &str,
        name: Option<String>,
    ) -> Result<(), AppServerError> {
        let mut next = self.clone();
        let thread = next.thread_mut(thread_id)?;
        thread.title = name;
        thread.updated_at = unix_timestamp();
        self.commit(next)
    }

    pub fn update_git_info(
        &mut self,
        thread_id: &str,
        git_info: Option<Value>,
    ) -> Result<(), AppServerError> {
        let mut next = self.clone();
        let thread = next.thread_mut(thread_id)?;
        thread.git_info = match git_info {
            Some(git_info) => Some(validated_git_info_patch(thread.git_info.clone(), git_info)?),
            None => None,
        };
        thread.updated_at = unix_timestamp();
        self.commit(next)
    }

    pub fn set_goal(
        &mut self,
        thread_id: &str,
        objective: String,
        status: Option<ThreadGoalStatus>,
        token_budget: Option<i64>,
    ) -> Result<ThreadGoal, AppServerError> {
        let now = unix_timestamp();
        let mut next = self.clone();
        let thread = next.thread_mut(thread_id)?;
        let previous = thread.goal.clone();
        let goal = ThreadGoal {
            thread_id: thread_id.to_string(),
            objective,
            status: status
                .or_else(|| previous.as_ref().map(|goal| goal.status.clone()))
                .unwrap_or(ThreadGoalStatus::Active),
            token_budget,
            tokens_used: previous.as_ref().map_or(0, |goal| goal.tokens_used),
            time_used_seconds: previous.as_ref().map_or(0, |goal| goal.time_used_seconds),
            created_at: previous.as_ref().map_or(now, |goal| goal.created_at),
            updated_at: now,
        };
        thread.goal = Some(goal.clone());
        thread.updated_at = now;
        self.commit(next)?;
        Ok(goal)
    }

    pub fn get_goal(&self, thread_id: &str) -> Result<Option<ThreadGoal>, AppServerError> {
        self.ensure_thread(thread_id)?;
        Ok(self
            .threads
            .iter()
            .find(|thread| thread.thread_id == thread_id)
            .and_then(|thread| thread.goal.clone()))
    }

    pub fn clear_goal(&mut self, thread_id: &str) -> Result<bool, AppServerError> {
        self.ensure_thread(thread_id)?;
        let mut next = self.clone();
        let thread = next.thread_mut(thread_id)?;
        if thread.goal.is_none() {
            return Ok(false);
        }
        thread.goal = None;
        thread.updated_at = unix_timestamp();
        self.commit(next)?;
        Ok(true)
    }

    pub fn apply_token_usage(
        &mut self,
        thread_id: &str,
        last: TokenUsageBreakdown,
    ) -> Result<ThreadTokenUsage, AppServerError> {
        let mut next = self.clone();
        let thread = next.thread_mut(thread_id)?;
        let previous_total = thread
            .token_usage
            .as_ref()
            .map(|usage| usage.total.clone())
            .unwrap_or_default();
        let total = TokenUsageBreakdown {
            total_tokens: previous_total
                .total_tokens
                .saturating_add(last.total_tokens),
            input_tokens: previous_total
                .input_tokens
                .saturating_add(last.input_tokens),
            cached_input_tokens: previous_total
                .cached_input_tokens
                .saturating_add(last.cached_input_tokens),
            output_tokens: previous_total
                .output_tokens
                .saturating_add(last.output_tokens),
            reasoning_output_tokens: previous_total
                .reasoning_output_tokens
                .saturating_add(last.reasoning_output_tokens),
        };
        let token_usage = ThreadTokenUsage {
            total: total.clone(),
            last,
            model_context_window: thread
                .token_usage
                .as_ref()
                .and_then(|usage| usage.model_context_window),
        };
        thread.token_usage = Some(token_usage.clone());
        if let Some(goal) = thread.goal.as_mut() {
            goal.tokens_used = i64::try_from(total.total_tokens).unwrap_or(i64::MAX);
            goal.updated_at = unix_timestamp();
        }
        thread.updated_at = unix_timestamp();
        self.commit(next)?;
        Ok(token_usage)
    }

    pub fn record_compacted_turn(
        &mut self,
        thread_id: &str,
        turn_id: String,
    ) -> Result<(), AppServerError> {
        let mut next = self.clone();
        let thread = next.thread_mut(thread_id)?;
        thread.compacted_turn_id = Some(turn_id);
        thread.updated_at = unix_timestamp();
        self.commit(next)
    }

    pub fn set_archived(&mut self, thread_id: &str, archived: bool) -> Result<(), AppServerError> {
        self.ensure_no_pending_turns(thread_id)?;
        let mut next = self.clone();
        let thread = next.thread_mut(thread_id)?;
        thread.archived = archived;
        if archived {
            thread.subscribed = false;
        }
        thread.updated_at = unix_timestamp();
        self.commit(next)
    }

    pub fn set_subscribed(
        &mut self,
        thread_id: &str,
        subscribed: bool,
    ) -> Result<bool, AppServerError> {
        let mut next = self.clone();
        let thread = next.thread_mut(thread_id)?;
        if thread.subscribed == subscribed {
            return Ok(false);
        }
        thread.subscribed = subscribed;
        thread.updated_at = unix_timestamp();
        self.commit(next)?;
        Ok(true)
    }

    pub fn fork(
        &mut self,
        source_thread_id: &str,
        params: ThreadFork,
    ) -> Result<String, AppServerError> {
        self.ensure_thread(source_thread_id)?;
        self.ensure_no_pending_turns(source_thread_id)?;
        let mut next = self.clone();
        let source = next
            .threads
            .iter()
            .find(|thread| thread.thread_id == source_thread_id)
            .cloned()
            .ok_or_else(|| {
                AppServerError::invalid_request(
                    "thread_lifecycle",
                    format!("unknown thread id: {source_thread_id}"),
                )
            })?;
        let thread_id = format!("thread_{}", next.next_thread_id);
        next.next_thread_id += 1;
        let now = unix_timestamp();
        next.threads.push(ThreadRecord {
            thread_id: thread_id.clone(),
            title: source.title,
            workspace_root: params.cwd.or(source.workspace_root),
            sandbox_context: source.sandbox_context,
            sandbox: source.sandbox,
            permission_profile: source.permission_profile,
            forked_from_id: Some(source_thread_id.to_string()),
            ephemeral: params.ephemeral,
            archived: false,
            subscribed: true,
            path: None,
            created_at: now,
            updated_at: now,
            git_info: source.git_info,
            goal: source.goal,
            compacted_turn_id: source.compacted_turn_id,
            token_usage: source.token_usage,
        });

        if !params.exclude_turns {
            let source_turns = next
                .turns
                .iter()
                .filter(|turn| turn.thread_id == source_thread_id)
                .cloned()
                .collect::<Vec<_>>();
            for source_turn in source_turns {
                let turn_id = format!("turn_{}", next.next_turn_id);
                next.next_turn_id += 1;
                next.turns.push(TurnRecord {
                    thread_id: thread_id.clone(),
                    turn_id,
                    status: source_turn.status,
                    output: source_turn.output,
                    items: source_turn.items,
                    error: source_turn.error,
                });
            }
        }

        self.commit(next)?;
        Ok(thread_id)
    }

    pub fn inject_items(
        &mut self,
        thread_id: &str,
        items: Vec<Value>,
    ) -> Result<String, AppServerError> {
        self.ensure_thread(thread_id)?;
        let mut next = self.clone();
        let target_index = next
            .turns
            .iter()
            .rposition(|turn| turn.thread_id == thread_id);
        let turn_id = match target_index {
            Some(index) => next.turns[index].turn_id.clone(),
            None => {
                let turn_id = format!("turn_{}", next.next_turn_id);
                next.next_turn_id += 1;
                next.turns.push(TurnRecord {
                    thread_id: thread_id.to_string(),
                    turn_id: turn_id.clone(),
                    status: TurnStatus::Completed,
                    output: None,
                    items: Vec::new(),
                    error: None,
                });
                turn_id
            }
        };
        let injected_items = items
            .into_iter()
            .enumerate()
            .map(|(index, value)| injected_codex_item(&turn_id, index, value))
            .collect::<Result<Vec<_>, _>>()?;
        let turn = next
            .turns
            .iter_mut()
            .find(|turn| turn.thread_id == thread_id && turn.turn_id == turn_id)
            .ok_or_else(|| {
                AppServerError::invalid_request(
                    "thread_lifecycle",
                    format!("unknown turn id: {turn_id}"),
                )
            })?;
        turn.items.extend(injected_items);
        next.touch_thread(thread_id)?;
        self.commit(next)?;
        Ok(turn_id)
    }

    #[must_use]
    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.threads.is_empty()
    }

    #[must_use]
    pub fn contains(&self, thread_id: &str) -> bool {
        self.threads
            .iter()
            .any(|thread| thread.thread_id == thread_id)
    }

    fn persist(&self) -> Result<(), AppServerError> {
        let Some(path) = self.snapshot_path.as_ref() else {
            return Ok(());
        };
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                thread_lifecycle_error(format!(
                    "failed to create thread lifecycle snapshot directory: {error}"
                ))
            })?;
        }

        let snapshot = ThreadLifecycleSnapshot {
            version: SNAPSHOT_VERSION,
            next_thread_id: self.next_thread_id,
            next_turn_id: self.next_turn_id,
            threads: self
                .threads
                .iter()
                .map(ThreadRecord::to_persisted)
                .collect(),
            turns: self.turns.iter().map(TurnRecord::to_persisted).collect(),
        };
        let content = serde_json::to_vec_pretty(&snapshot).map_err(|error| {
            thread_lifecycle_error(format!(
                "failed to serialize thread lifecycle snapshot: {error}"
            ))
        })?;
        let tmp_path = tmp_snapshot_path(path);
        {
            let mut file = File::create(&tmp_path).map_err(|error| {
                thread_lifecycle_error(format!(
                    "failed to create temporary thread lifecycle snapshot: {error}"
                ))
            })?;
            file.write_all(&content).map_err(|error| {
                thread_lifecycle_error(format!(
                    "failed to write temporary thread lifecycle snapshot: {error}"
                ))
            })?;
            file.flush().map_err(|error| {
                thread_lifecycle_error(format!(
                    "failed to flush temporary thread lifecycle snapshot: {error}"
                ))
            })?;
        }
        fs::rename(&tmp_path, path).map_err(|error| {
            thread_lifecycle_error(format!(
                "failed to replace thread lifecycle snapshot: {error}"
            ))
        })
    }

    fn commit(&mut self, next: Self) -> Result<(), AppServerError> {
        next.persist()?;
        *self = next;
        Ok(())
    }

    fn thread_mut(&mut self, thread_id: &str) -> Result<&mut ThreadRecord, AppServerError> {
        self.threads
            .iter_mut()
            .find(|thread| thread.thread_id == thread_id)
            .ok_or_else(|| {
                AppServerError::invalid_request(
                    "thread_lifecycle",
                    format!("unknown thread id: {thread_id}"),
                )
            })
    }

    fn ensure_thread(&self, thread_id: &str) -> Result<(), AppServerError> {
        self.threads
            .iter()
            .any(|thread| thread.thread_id == thread_id)
            .then_some(())
            .ok_or_else(|| {
                AppServerError::invalid_request(
                    "thread_lifecycle",
                    format!("unknown thread id: {thread_id}"),
                )
            })
    }

    fn ensure_no_pending_turns(&self, thread_id: &str) -> Result<(), AppServerError> {
        self.ensure_thread(thread_id)?;
        if self
            .turns
            .iter()
            .any(|turn| turn.thread_id == thread_id && turn.status == TurnStatus::Pending)
        {
            return Err(AppServerError::invalid_request(
                "thread_lifecycle",
                "cannot mutate lifecycle while thread has pending turns",
            ));
        }
        Ok(())
    }

    fn touch_thread(&mut self, thread_id: &str) -> Result<(), AppServerError> {
        let thread = self.thread_mut(thread_id)?;
        thread.updated_at = unix_timestamp();
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThreadCreation {
    pub cwd: Option<String>,
    pub sandbox: Option<SandboxMode>,
    pub permission_profile: Option<Value>,
    pub title: Option<String>,
    pub ephemeral: bool,
    pub forked_from_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThreadFork {
    pub cwd: Option<String>,
    pub ephemeral: bool,
    pub exclude_turns: bool,
}

impl ThreadCreation {
    #[cfg(test)]
    pub fn in_memory_for_test() -> Self {
        Self {
            cwd: None,
            sandbox: None,
            permission_profile: None,
            title: None,
            ephemeral: true,
            forked_from_id: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThreadRecord {
    pub thread_id: String,
    pub title: Option<String>,
    pub workspace_root: Option<String>,
    pub sandbox_context: RuntimeSandboxContext,
    pub sandbox: Option<SandboxMode>,
    pub permission_profile: Option<Value>,
    pub forked_from_id: Option<String>,
    pub ephemeral: bool,
    pub archived: bool,
    pub subscribed: bool,
    pub path: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub git_info: Option<CodexGitInfo>,
    pub goal: Option<ThreadGoal>,
    pub compacted_turn_id: Option<String>,
    pub token_usage: Option<ThreadTokenUsage>,
}

impl ThreadRecord {
    fn to_summary(&self) -> ThreadSummary {
        ThreadSummary {
            thread_id: self.thread_id.clone(),
            title: self.title.clone(),
            workspace_root: self.workspace_root.clone(),
            sandbox_context: self.sandbox_context.clone(),
            sandbox: self.sandbox,
            permission_profile: self.permission_profile.clone(),
            forked_from_id: self.forked_from_id.clone(),
            ephemeral: self.ephemeral,
            archived: self.archived,
            subscribed: self.subscribed,
            path: self.path.clone(),
            created_at: self.created_at,
            updated_at: self.updated_at,
            git_info: self.git_info.clone(),
            goal: self.goal.clone(),
            compacted_turn_id: self.compacted_turn_id.clone(),
            token_usage: self.token_usage.clone(),
        }
    }

    fn to_persisted(&self) -> PersistedThreadRecord {
        PersistedThreadRecord {
            thread_id: self.thread_id.clone(),
            title: self.title.clone(),
            workspace_root: self.workspace_root.clone(),
            sandbox: self.sandbox,
            permission_profile: self.permission_profile.clone(),
            forked_from_id: self.forked_from_id.clone(),
            ephemeral: self.ephemeral,
            archived: self.archived,
            subscribed: self.subscribed,
            path: self.path.clone(),
            created_at: self.created_at,
            updated_at: self.updated_at,
            git_info: self.git_info.clone(),
            goal: self.goal.clone(),
            compacted_turn_id: self.compacted_turn_id.clone(),
            token_usage: self.token_usage.clone(),
        }
    }

    fn from_persisted(record: PersistedThreadRecord) -> Result<Self, AppServerError> {
        let cwd = cwd_path(record.workspace_root.as_deref());
        let sandbox_context = crate::sandbox_protocol::resolve_thread_context(
            record.sandbox,
            record.permission_profile.clone(),
            cwd,
            "thread/lifecycle/load",
        )?;
        Ok(Self {
            thread_id: record.thread_id,
            title: record.title,
            workspace_root: record.workspace_root,
            sandbox_context,
            sandbox: record.sandbox,
            permission_profile: record.permission_profile,
            forked_from_id: record.forked_from_id,
            ephemeral: record.ephemeral,
            archived: record.archived,
            subscribed: record.subscribed,
            path: record.path,
            created_at: record.created_at,
            updated_at: record.updated_at,
            git_info: record.git_info,
            goal: record.goal,
            compacted_turn_id: record.compacted_turn_id,
            token_usage: record.token_usage,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThreadSummary {
    pub thread_id: String,
    pub title: Option<String>,
    pub workspace_root: Option<String>,
    pub sandbox_context: RuntimeSandboxContext,
    pub sandbox: Option<SandboxMode>,
    pub permission_profile: Option<Value>,
    pub forked_from_id: Option<String>,
    pub ephemeral: bool,
    pub archived: bool,
    pub subscribed: bool,
    pub path: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub git_info: Option<CodexGitInfo>,
    pub goal: Option<ThreadGoal>,
    pub compacted_turn_id: Option<String>,
    pub token_usage: Option<ThreadTokenUsage>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TurnRecord {
    pub thread_id: String,
    pub turn_id: String,
    pub status: TurnStatus,
    pub output: Option<String>,
    pub items: Vec<CodexThreadItem>,
    pub error: Option<String>,
}

impl TurnRecord {
    fn to_summary(&self) -> TurnSummary {
        TurnSummary {
            thread_id: self.thread_id.clone(),
            turn_id: self.turn_id.clone(),
            status: self.status,
            output: self.output.clone(),
            items: self.items.clone(),
            error: self.error.clone(),
        }
    }

    fn to_persisted(&self) -> PersistedTurnRecord {
        PersistedTurnRecord {
            thread_id: self.thread_id.clone(),
            turn_id: self.turn_id.clone(),
            status: self.status,
            output: self.output.clone(),
            items: self.items.clone(),
            error: self.error.clone(),
        }
    }
}

impl From<PersistedTurnRecord> for TurnRecord {
    fn from(record: PersistedTurnRecord) -> Self {
        Self {
            thread_id: record.thread_id,
            turn_id: record.turn_id,
            status: record.status,
            output: record.output,
            items: record.items,
            error: record.error,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TurnSummary {
    pub thread_id: String,
    pub turn_id: String,
    pub status: TurnStatus,
    pub output: Option<String>,
    pub items: Vec<CodexThreadItem>,
    pub error: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ThreadLifecycleSnapshot {
    version: u32,
    next_thread_id: u64,
    next_turn_id: u64,
    threads: Vec<PersistedThreadRecord>,
    turns: Vec<PersistedTurnRecord>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PersistedThreadRecord {
    thread_id: String,
    title: Option<String>,
    workspace_root: Option<String>,
    sandbox: Option<SandboxMode>,
    permission_profile: Option<Value>,
    forked_from_id: Option<String>,
    ephemeral: bool,
    archived: bool,
    subscribed: bool,
    path: Option<String>,
    created_at: i64,
    updated_at: i64,
    git_info: Option<CodexGitInfo>,
    goal: Option<ThreadGoal>,
    compacted_turn_id: Option<String>,
    token_usage: Option<ThreadTokenUsage>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PersistedTurnRecord {
    thread_id: String,
    turn_id: String,
    status: TurnStatus,
    output: Option<String>,
    #[serde(default)]
    items: Vec<CodexThreadItem>,
    error: Option<String>,
}

fn cwd_path(cwd: Option<&str>) -> &Path {
    cwd.map(Path::new).unwrap_or_else(|| Path::new("."))
}

fn tmp_snapshot_path(path: &Path) -> PathBuf {
    let mut name = path
        .file_name()
        .map(|name| name.to_os_string())
        .unwrap_or_else(|| "threads.json".into());
    name.push(".tmp");
    path.with_file_name(name)
}

fn unix_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

fn thread_lifecycle_error(message: impl Into<String>) -> AppServerError {
    AppServerError::service_degraded("thread_lifecycle", message)
}

fn injected_codex_item(
    turn_id: &str,
    index: usize,
    value: Value,
) -> Result<CodexThreadItem, AppServerError> {
    let Value::Object(fields) = value else {
        return Err(AppServerError::invalid_request(
            "thread_lifecycle",
            "injected item must be an object",
        ));
    };
    let item_type = fields
        .get("type")
        .and_then(Value::as_str)
        .ok_or_else(|| AppServerError::invalid_request("thread_lifecycle", "missing item type"))?;
    let id = fields.get("id").and_then(Value::as_str).map_or_else(
        || format!("{turn_id}:injected:{index}"),
        ToString::to_string,
    );
    match item_type {
        "agentMessage" => Ok(CodexThreadItem::AgentMessage {
            id,
            text: fields
                .get("text")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
        }),
        "reasoning" => Ok(CodexThreadItem::Reasoning {
            id,
            summary: string_array_field(&fields, "summary")?,
            content: string_array_field(&fields, "content")?,
        }),
        _ => Err(AppServerError::invalid_request(
            "thread_lifecycle",
            format!("unsupported injected item type: {item_type}"),
        )),
    }
}

fn string_array_field(
    fields: &serde_json::Map<String, Value>,
    name: &str,
) -> Result<Vec<String>, AppServerError> {
    let Some(value) = fields.get(name) else {
        return Ok(Vec::new());
    };
    let Value::Array(values) = value else {
        return Err(AppServerError::invalid_request(
            "thread_lifecycle",
            format!("{name} must be an array of strings"),
        ));
    };
    values
        .iter()
        .map(|value| {
            value.as_str().map(ToString::to_string).ok_or_else(|| {
                AppServerError::invalid_request(
                    "thread_lifecycle",
                    format!("{name} must be an array of strings"),
                )
            })
        })
        .collect()
}

fn validated_git_info_patch(
    current: Option<CodexGitInfo>,
    patch: Value,
) -> Result<CodexGitInfo, AppServerError> {
    let Value::Object(fields) = patch else {
        return Err(AppServerError::invalid_request(
            "thread_lifecycle",
            "gitInfo must be an object or null",
        ));
    };
    let mut git_info = current.unwrap_or(CodexGitInfo {
        sha: None,
        branch: None,
        origin_url: None,
    });
    for (key, value) in fields {
        let target = match key.as_str() {
            "sha" => &mut git_info.sha,
            "branch" => &mut git_info.branch,
            "originUrl" => &mut git_info.origin_url,
            _ => {
                return Err(AppServerError::invalid_request(
                    "thread_lifecycle",
                    format!("unsupported gitInfo field: {key}"),
                ));
            }
        };
        *target = match value {
            Value::Null => None,
            Value::String(value) if !value.is_empty() => Some(value),
            Value::String(_) => {
                return Err(AppServerError::invalid_request(
                    "thread_lifecycle",
                    format!("gitInfo.{key} must not be empty"),
                ));
            }
            _ => {
                return Err(AppServerError::invalid_request(
                    "thread_lifecycle",
                    format!("gitInfo.{key} must be a string or null"),
                ));
            }
        };
    }
    Ok(git_info)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thread_lifecycle_persists_archive_name_git_info_and_goal() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("threads.json");
        let mut host =
            ThreadLifecycleHost::with_snapshot_path(path.clone()).expect("create persistent host");

        let thread_id = host
            .create(ThreadCreation {
                cwd: Some("/workspace".to_string()),
                sandbox: None,
                permission_profile: None,
                title: Some("Initial".to_string()),
                ephemeral: false,
                forked_from_id: None,
            })
            .expect("create thread");
        host.set_name(&thread_id, Some("Renamed".to_string()))
            .expect("rename");
        host.update_git_info(
            &thread_id,
            Some(serde_json::json!({"sha": "abc123", "branch": "main", "originUrl": null})),
        )
        .expect("metadata update");
        host.set_goal(&thread_id, "Ship R4".to_string(), None, Some(5000))
            .expect("set goal");
        host.set_archived(&thread_id, true).expect("archive");

        let reloaded = ThreadLifecycleHost::with_snapshot_path(path).expect("reload");
        let summary = reloaded.summary(&thread_id).expect("summary");
        assert_eq!(summary.title.as_deref(), Some("Renamed"));
        assert!(summary.archived);
        assert_eq!(
            summary.git_info.expect("git info").sha.as_deref(),
            Some("abc123")
        );
        assert_eq!(summary.goal.expect("goal").objective, "Ship R4");
    }

    #[test]
    fn thread_lifecycle_rolls_back_turns_without_reusing_ids() {
        let mut host = ThreadLifecycleHost::new();
        let thread_id = host
            .create(ThreadCreation::in_memory_for_test())
            .expect("create");
        let first = host.next_turn_id();
        host.record_started_turn(&thread_id, first.clone())
            .expect("record first turn");
        host.apply_runtime_turn_update(RuntimeTurnUpdate {
            thread_id: thread_id.clone(),
            turn_id: first,
            outcome: RuntimeTurnOutcome::Completed {
                output: "one".to_string(),
            },
        })
        .expect("apply update");
        let second = host.next_turn_id();
        host.record_started_turn(&thread_id, second.clone())
            .expect("record second turn");

        let error = host
            .rollback(&thread_id, 1)
            .expect_err("pending turn rollback is rejected");
        assert!(
            error.public_message().contains("pending"),
            "unexpected error: {error:?}"
        );
        assert_eq!(host.list_turns(&thread_id).len(), 2);
        assert_eq!(host.next_turn_id(), "turn_3");

        host.apply_runtime_turn_update(RuntimeTurnUpdate {
            thread_id: thread_id.clone(),
            turn_id: second,
            outcome: RuntimeTurnOutcome::Completed {
                output: "two".to_string(),
            },
        })
        .expect("complete second turn");
        let removed = host.rollback(&thread_id, 1).expect("rollback");
        assert_eq!(removed, 1);
        assert_eq!(host.list_turns(&thread_id).len(), 1);
        assert_eq!(host.next_turn_id(), "turn_3");
    }

    #[test]
    fn thread_lifecycle_rejects_unknown_threads_without_consuming_turn_ids() {
        let mut host = ThreadLifecycleHost::new();

        let error = host
            .record_started_turn("missing", "turn_1".to_string())
            .expect_err("unknown thread is rejected");
        assert!(
            error.public_message().contains("unknown thread id"),
            "unexpected error: {error:?}"
        );
        assert_eq!(host.next_turn_id(), "turn_1");
        assert!(host.list_turns("missing").is_empty());

        let error = host
            .rollback("missing", 1)
            .expect_err("unknown thread rollback is rejected");
        assert!(
            error.public_message().contains("unknown thread id"),
            "unexpected error: {error:?}"
        );
        assert_eq!(host.next_turn_id(), "turn_1");
    }

    #[test]
    fn thread_lifecycle_persist_failure_does_not_commit_memory_state() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("threads.json");
        let mut host =
            ThreadLifecycleHost::with_snapshot_path(path.clone()).expect("create persistent host");
        fs::create_dir(&path).expect("force snapshot path to be a directory");

        host.create(ThreadCreation::in_memory_for_test())
            .expect_err("persist failure rejects create");
        assert!(host.is_empty());

        fs::remove_dir(&path).expect("remove blocking directory");
        let thread_id = host
            .create(ThreadCreation::in_memory_for_test())
            .expect("create after path is writable");
        assert_eq!(thread_id, "thread_1");
    }
}
