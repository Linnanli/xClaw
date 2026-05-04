//! In-memory registries for Team and Cron lifecycle management.
//!
//! Ported from `claw-code/rust/crates/runtime/src/team_cron_registry.rs` (509 LOC).
//!
//! Provides `TeamCreate/Delete` and `CronCreate/Delete/List` runtime backing
//! to replace stub implementations in upstream `tools` crate.
//!
//! Issue: #200 (W5 task lifecycle 3-pack, parallel with #198/#199).
//!
//! ## Lock-poison handling
//!
//! Upstream uses `.expect("... lock poisoned")` which violates xClaw's
//! `check_no_panics` red-line. Here we recover from poisoned locks via
//! `.unwrap_or_else(|p| p.into_inner())`: a poisoned lock means a previous
//! holder panicked, but the inner data is still well-formed since each
//! mutator only takes the lock for the duration of a single update. This
//! gives Fail-Safe behaviour (continue best-effort) instead of Fail-Open
//! (allow panics to propagate).

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Team {
    pub team_id: String,
    pub name: String,
    pub task_ids: Vec<String>,
    pub status: TeamStatus,
    pub created_at: u64,
    pub updated_at: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TeamStatus {
    Created,
    Running,
    Completed,
    Deleted,
}

impl std::fmt::Display for TeamStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Created => write!(f, "created"),
            Self::Running => write!(f, "running"),
            Self::Completed => write!(f, "completed"),
            Self::Deleted => write!(f, "deleted"),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct TeamRegistry {
    inner: Arc<Mutex<TeamInner>>,
}

#[derive(Debug, Default)]
struct TeamInner {
    teams: HashMap<String, Team>,
    counter: u64,
}

/// Lock helper that recovers from poisoning instead of panicking.
fn lock_team(m: &Mutex<TeamInner>) -> MutexGuard<'_, TeamInner> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

impl TeamRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn create(&self, name: &str, task_ids: Vec<String>) -> Team {
        let mut inner = lock_team(&self.inner);
        inner.counter += 1;
        let ts = now_secs();
        let team_id = format!("team_{:08x}_{}", ts, inner.counter);
        let team = Team {
            team_id: team_id.clone(),
            name: name.to_owned(),
            task_ids,
            status: TeamStatus::Created,
            created_at: ts,
            updated_at: ts,
        };
        inner.teams.insert(team_id, team.clone());
        team
    }

    pub fn get(&self, team_id: &str) -> Option<Team> {
        let inner = lock_team(&self.inner);
        inner.teams.get(team_id).cloned()
    }

    pub fn list(&self) -> Vec<Team> {
        let inner = lock_team(&self.inner);
        inner.teams.values().cloned().collect()
    }

    pub fn delete(&self, team_id: &str) -> Result<Team, String> {
        let mut inner = lock_team(&self.inner);
        let team = inner
            .teams
            .get_mut(team_id)
            .ok_or_else(|| format!("team not found: {team_id}"))?;
        team.status = TeamStatus::Deleted;
        team.updated_at = now_secs();
        Ok(team.clone())
    }

    pub fn remove(&self, team_id: &str) -> Option<Team> {
        let mut inner = lock_team(&self.inner);
        inner.teams.remove(team_id)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        let inner = lock_team(&self.inner);
        inner.teams.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CronEntry {
    pub cron_id: String,
    pub schedule: String,
    pub prompt: String,
    pub description: Option<String>,
    pub enabled: bool,
    pub created_at: u64,
    pub updated_at: u64,
    pub last_run_at: Option<u64>,
    pub run_count: u64,
}

#[derive(Debug, Clone, Default)]
pub struct CronRegistry {
    inner: Arc<Mutex<CronInner>>,
}

#[derive(Debug, Default)]
struct CronInner {
    entries: HashMap<String, CronEntry>,
    counter: u64,
}

fn lock_cron(m: &Mutex<CronInner>) -> MutexGuard<'_, CronInner> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

impl CronRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn create(&self, schedule: &str, prompt: &str, description: Option<&str>) -> CronEntry {
        let mut inner = lock_cron(&self.inner);
        inner.counter += 1;
        let ts = now_secs();
        let cron_id = format!("cron_{:08x}_{}", ts, inner.counter);
        let entry = CronEntry {
            cron_id: cron_id.clone(),
            schedule: schedule.to_owned(),
            prompt: prompt.to_owned(),
            description: description.map(str::to_owned),
            enabled: true,
            created_at: ts,
            updated_at: ts,
            last_run_at: None,
            run_count: 0,
        };
        inner.entries.insert(cron_id, entry.clone());
        entry
    }

    pub fn get(&self, cron_id: &str) -> Option<CronEntry> {
        let inner = lock_cron(&self.inner);
        inner.entries.get(cron_id).cloned()
    }

    pub fn list(&self, enabled_only: bool) -> Vec<CronEntry> {
        let inner = lock_cron(&self.inner);
        inner
            .entries
            .values()
            .filter(|e| !enabled_only || e.enabled)
            .cloned()
            .collect()
    }

    pub fn delete(&self, cron_id: &str) -> Result<CronEntry, String> {
        let mut inner = lock_cron(&self.inner);
        inner
            .entries
            .remove(cron_id)
            .ok_or_else(|| format!("cron not found: {cron_id}"))
    }

    /// Disable a cron entry without removing it.
    pub fn disable(&self, cron_id: &str) -> Result<(), String> {
        let mut inner = lock_cron(&self.inner);
        let entry = inner
            .entries
            .get_mut(cron_id)
            .ok_or_else(|| format!("cron not found: {cron_id}"))?;
        entry.enabled = false;
        entry.updated_at = now_secs();
        Ok(())
    }

    /// Record a cron run.
    pub fn record_run(&self, cron_id: &str) -> Result<(), String> {
        let mut inner = lock_cron(&self.inner);
        let entry = inner
            .entries
            .get_mut(cron_id)
            .ok_or_else(|| format!("cron not found: {cron_id}"))?;
        entry.last_run_at = Some(now_secs());
        entry.run_count += 1;
        entry.updated_at = now_secs();
        Ok(())
    }

    #[must_use]
    pub fn len(&self) -> usize {
        let inner = lock_cron(&self.inner);
        inner.entries.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Team tests ──────────────────────────────────────

    #[test]
    fn req_team_cron_200_creates_and_retrieves_team() {
        let registry = TeamRegistry::new();
        let team = registry.create("Alpha Squad", vec!["task_001".into(), "task_002".into()]);
        assert_eq!(team.name, "Alpha Squad");
        assert_eq!(team.task_ids.len(), 2);
        assert_eq!(team.status, TeamStatus::Created);

        let fetched = registry.get(&team.team_id).expect("team should exist");
        assert_eq!(fetched.team_id, team.team_id);
    }

    #[test]
    fn req_team_cron_200_lists_and_deletes_teams() {
        let registry = TeamRegistry::new();
        let t1 = registry.create("Team A", vec![]);
        let t2 = registry.create("Team B", vec![]);

        let all = registry.list();
        assert_eq!(all.len(), 2);

        let deleted = registry.delete(&t1.team_id).expect("delete should succeed");
        assert_eq!(deleted.status, TeamStatus::Deleted);

        let still_there = registry
            .get(&t1.team_id)
            .expect("soft-deleted team listable");
        assert_eq!(still_there.status, TeamStatus::Deleted);

        registry.remove(&t2.team_id);
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn req_team_cron_200_rejects_missing_team_operations() {
        let registry = TeamRegistry::new();
        assert!(registry.delete("nonexistent").is_err());
        assert!(registry.get("nonexistent").is_none());
    }

    #[test]
    fn req_team_cron_200_team_status_display_all_variants() {
        assert_eq!(TeamStatus::Created.to_string(), "created");
        assert_eq!(TeamStatus::Running.to_string(), "running");
        assert_eq!(TeamStatus::Completed.to_string(), "completed");
        assert_eq!(TeamStatus::Deleted.to_string(), "deleted");
    }

    #[test]
    fn req_team_cron_200_new_team_registry_is_empty() {
        let registry = TeamRegistry::new();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);
        assert!(registry.list().is_empty());
    }

    #[test]
    fn req_team_cron_200_team_remove_nonexistent_returns_none() {
        let registry = TeamRegistry::new();
        assert!(registry.remove("missing").is_none());
    }

    #[test]
    fn req_team_cron_200_team_len_transitions() {
        let registry = TeamRegistry::new();
        let alpha = registry.create("Alpha", vec![]);
        let beta = registry.create("Beta", vec![]);
        assert_eq!(registry.len(), 2);
        registry.remove(&alpha.team_id);
        assert_eq!(registry.len(), 1);
        registry.remove(&beta.team_id);
        assert_eq!(registry.len(), 0);
        assert!(registry.is_empty());
    }

    #[test]
    fn req_team_cron_200_team_serialization_roundtrip() {
        let registry = TeamRegistry::new();
        let team = registry.create("Squad", vec!["t1".into()]);
        let json = serde_json::to_string(&team).expect("serialize");
        let back: Team = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.team_id, team.team_id);
        assert_eq!(back.status, TeamStatus::Created);
    }

    // ── Cron tests ──────────────────────────────────────

    #[test]
    fn req_team_cron_200_creates_and_retrieves_cron() {
        let registry = CronRegistry::new();
        let entry = registry.create("0 * * * *", "Check status", Some("hourly check"));
        assert_eq!(entry.schedule, "0 * * * *");
        assert_eq!(entry.prompt, "Check status");
        assert!(entry.enabled);
        assert_eq!(entry.run_count, 0);
        assert!(entry.last_run_at.is_none());

        let fetched = registry.get(&entry.cron_id).expect("cron should exist");
        assert_eq!(fetched.cron_id, entry.cron_id);
    }

    #[test]
    fn req_team_cron_200_lists_with_enabled_filter() {
        let registry = CronRegistry::new();
        let c1 = registry.create("* * * * *", "Task 1", None);
        let c2 = registry.create("0 * * * *", "Task 2", None);
        registry.disable(&c1.cron_id).expect("disable");

        let all = registry.list(false);
        assert_eq!(all.len(), 2);

        let enabled_only = registry.list(true);
        assert_eq!(enabled_only.len(), 1);
        assert_eq!(enabled_only[0].cron_id, c2.cron_id);
    }

    #[test]
    fn req_team_cron_200_deletes_cron_entry() {
        let registry = CronRegistry::new();
        let entry = registry.create("* * * * *", "To delete", None);
        let deleted = registry.delete(&entry.cron_id).expect("delete");
        assert_eq!(deleted.cron_id, entry.cron_id);
        assert!(registry.get(&entry.cron_id).is_none());
        assert!(registry.is_empty());
    }

    #[test]
    fn req_team_cron_200_records_cron_runs() {
        let registry = CronRegistry::new();
        let entry = registry.create("*/5 * * * *", "Recurring", None);
        registry.record_run(&entry.cron_id).expect("first run");
        registry.record_run(&entry.cron_id).expect("second run");

        let fetched = registry.get(&entry.cron_id).expect("entry exists");
        assert_eq!(fetched.run_count, 2);
        assert!(fetched.last_run_at.is_some());
    }

    #[test]
    fn req_team_cron_200_rejects_missing_cron_operations() {
        let registry = CronRegistry::new();
        assert!(registry.delete("nonexistent").is_err());
        assert!(registry.disable("nonexistent").is_err());
        assert!(registry.record_run("nonexistent").is_err());
        assert!(registry.get("nonexistent").is_none());
    }

    #[test]
    fn req_team_cron_200_cron_list_all_disabled_returns_empty_for_enabled_only() {
        let registry = CronRegistry::new();
        let first = registry.create("* * * * *", "Task 1", None);
        let second = registry.create("0 * * * *", "Task 2", None);
        registry.disable(&first.cron_id).expect("disable first");
        registry.disable(&second.cron_id).expect("disable second");

        assert!(registry.list(true).is_empty());
        assert_eq!(registry.list(false).len(), 2);
    }

    #[test]
    fn req_team_cron_200_cron_create_without_description() {
        let registry = CronRegistry::new();
        let entry = registry.create("*/15 * * * *", "Check health", None);
        assert!(entry.cron_id.starts_with("cron_"));
        assert_eq!(entry.description, None);
        assert!(entry.enabled);
        assert_eq!(entry.run_count, 0);
        assert_eq!(entry.last_run_at, None);
    }

    #[test]
    fn req_team_cron_200_new_cron_registry_is_empty() {
        let registry = CronRegistry::new();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);
        assert!(registry.list(true).is_empty());
        assert!(registry.list(false).is_empty());
    }

    #[test]
    fn req_team_cron_200_cron_record_run_updates_timestamp_and_counter() {
        let registry = CronRegistry::new();
        let entry = registry.create("*/5 * * * *", "Recurring", None);

        registry.record_run(&entry.cron_id).expect("first run");
        registry.record_run(&entry.cron_id).expect("second run");
        let fetched = registry.get(&entry.cron_id).expect("entry exists");

        assert_eq!(fetched.run_count, 2);
        assert!(fetched.last_run_at.is_some());
        assert!(fetched.updated_at >= entry.updated_at);
    }

    #[test]
    fn req_team_cron_200_cron_disable_updates_timestamp() {
        let registry = CronRegistry::new();
        let entry = registry.create("0 0 * * *", "Nightly", None);

        registry.disable(&entry.cron_id).expect("disable");
        let fetched = registry.get(&entry.cron_id).expect("entry exists");

        assert!(!fetched.enabled);
        assert!(fetched.updated_at >= entry.updated_at);
    }

    #[test]
    fn req_team_cron_200_cron_serialization_roundtrip() {
        let registry = CronRegistry::new();
        let entry = registry.create("0 9 * * 1", "Weekly digest", Some("Mondays 9am"));
        let json = serde_json::to_string(&entry).expect("serialize");
        let back: CronEntry = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.cron_id, entry.cron_id);
        assert_eq!(back.description.as_deref(), Some("Mondays 9am"));
    }

    #[test]
    fn req_team_cron_200_team_status_serde_snake_case() {
        let json = serde_json::to_string(&TeamStatus::Completed).expect("serialize");
        assert_eq!(json, "\"completed\"");
        let parsed: TeamStatus = serde_json::from_str("\"deleted\"").expect("deserialize");
        assert_eq!(parsed, TeamStatus::Deleted);
    }

    #[test]
    fn req_team_cron_200_lock_recovers_from_poison() {
        // Poison the team registry by panicking inside a lock.
        let registry = TeamRegistry::new();
        let registry_clone = registry.clone();
        let _ = std::thread::spawn(move || {
            let _guard = registry_clone.inner.lock().expect("lock");
            panic!("intentional panic to poison");
        })
        .join();

        // Despite poisoning, subsequent operations succeed.
        let team = registry.create("Phoenix", vec![]);
        assert_eq!(team.status, TeamStatus::Created);
        assert_eq!(registry.len(), 1);
    }
}
