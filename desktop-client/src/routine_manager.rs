use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RoutineTrigger {
    Time(String),           // Cron expression: "0 9 * * *" (daily at 9 AM)
    Event(String),          // Event name: "file_changed", "message_received"
    Manual,                 // Manual trigger only
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RoutineStatus {
    Active,
    Paused,
    Disabled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutineAction {
    pub id: String,
    pub action_type: String,  // "send_message", "run_skill", "call_api"
    pub parameters: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Routine {
    pub id: String,
    pub name: String,
    pub description: String,
    pub trigger: RoutineTrigger,
    pub actions: Vec<RoutineAction>,
    pub status: RoutineStatus,
    pub created_at: i64,
    pub updated_at: i64,
    pub last_run: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutineRun {
    pub id: String,
    pub routine_id: String,
    pub status: String,  // "success", "failed", "running"
    pub started_at: i64,
    pub completed_at: Option<i64>,
    pub error: Option<String>,
}

pub struct RoutineManager {
    routines: std::collections::HashMap<String, Routine>,
    runs: Vec<RoutineRun>,
}

impl RoutineManager {
    pub fn new() -> Self {
        Self {
            routines: std::collections::HashMap::new(),
            runs: Vec::new(),
        }
    }

    pub fn create_routine(
        &mut self,
        name: String,
        description: String,
        trigger: RoutineTrigger,
        actions: Vec<RoutineAction>,
    ) -> Result<Routine> {
        let id = Uuid::new_v4().to_string();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let routine = Routine {
            id: id.clone(),
            name,
            description,
            trigger,
            actions,
            status: RoutineStatus::Active,
            created_at: now,
            updated_at: now,
            last_run: None,
        };

        self.routines.insert(id, routine.clone());
        Ok(routine)
    }

    pub fn get_routine(&self, routine_id: &str) -> Option<&Routine> {
        self.routines.get(routine_id)
    }

    pub fn get_all_routines(&self) -> Vec<&Routine> {
        self.routines.values().collect()
    }

    pub fn update_routine(&mut self, routine_id: &str, routine: Routine) -> Result<()> {
        if !self.routines.contains_key(routine_id) {
            return Err(Error::StorageError("Routine not found".to_string()));
        }

        self.routines.insert(routine_id.to_string(), routine);
        Ok(())
    }

    pub fn delete_routine(&mut self, routine_id: &str) -> Result<()> {
        self.routines
            .remove(routine_id)
            .ok_or(Error::StorageError("Routine not found".to_string()))?;
        Ok(())
    }

    pub fn enable_routine(&mut self, routine_id: &str) -> Result<()> {
        let routine = self
            .routines
            .get_mut(routine_id)
            .ok_or(Error::StorageError("Routine not found".to_string()))?;
        routine.status = RoutineStatus::Active;
        Ok(())
    }

    pub fn disable_routine(&mut self, routine_id: &str) -> Result<()> {
        let routine = self
            .routines
            .get_mut(routine_id)
            .ok_or(Error::StorageError("Routine not found".to_string()))?;
        routine.status = RoutineStatus::Disabled;
        Ok(())
    }

    pub fn pause_routine(&mut self, routine_id: &str) -> Result<()> {
        let routine = self
            .routines
            .get_mut(routine_id)
            .ok_or(Error::StorageError("Routine not found".to_string()))?;
        routine.status = RoutineStatus::Paused;
        Ok(())
    }

    pub fn trigger_routine(&mut self, routine_id: &str) -> Result<RoutineRun> {
        let routine = self
            .routines
            .get_mut(routine_id)
            .ok_or(Error::StorageError("Routine not found".to_string()))?;

        if routine.status == RoutineStatus::Disabled {
            return Err(Error::StorageError("Routine is disabled".to_string()));
        }

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let run = RoutineRun {
            id: Uuid::new_v4().to_string(),
            routine_id: routine_id.to_string(),
            status: "running".to_string(),
            started_at: now,
            completed_at: None,
            error: None,
        };

        routine.last_run = Some(now);
        self.runs.push(run.clone());
        Ok(run)
    }

    pub fn complete_routine_run(&mut self, run_id: &str, success: bool, error: Option<String>) -> Result<()> {
        let run = self
            .runs
            .iter_mut()
            .find(|r| r.id == run_id)
            .ok_or(Error::StorageError("Run not found".to_string()))?;

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        run.completed_at = Some(now);
        run.status = if success { "success".to_string() } else { "failed".to_string() };
        run.error = error;

        Ok(())
    }

    pub fn get_routine_runs(&self, routine_id: &str, limit: usize) -> Vec<&RoutineRun> {
        self.runs
            .iter()
            .filter(|r| r.routine_id == routine_id)
            .rev()
            .take(limit)
            .collect()
    }

    pub fn get_active_routines(&self) -> Vec<&Routine> {
        self.routines
            .values()
            .filter(|r| r.status == RoutineStatus::Active)
            .collect()
    }
}

impl Default for RoutineManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_routine() {
        let mut manager = RoutineManager::new();
        let routine = manager
            .create_routine(
                "Daily Report".to_string(),
                "Generate daily report".to_string(),
                RoutineTrigger::Time("0 9 * * *".to_string()),
                vec![],
            )
            .unwrap();

        assert_eq!(routine.name, "Daily Report");
        assert_eq!(routine.status, RoutineStatus::Active);
    }

    #[test]
    fn test_trigger_routine() {
        let mut manager = RoutineManager::new();
        let routine = manager
            .create_routine(
                "Test".to_string(),
                "Test routine".to_string(),
                RoutineTrigger::Manual,
                vec![],
            )
            .unwrap();

        let run = manager.trigger_routine(&routine.id).unwrap();
        assert_eq!(run.status, "running");
        assert_eq!(run.routine_id, routine.id);
    }

    #[test]
    fn test_disable_routine() {
        let mut manager = RoutineManager::new();
        let routine = manager
            .create_routine(
                "Test".to_string(),
                "Test routine".to_string(),
                RoutineTrigger::Manual,
                vec![],
            )
            .unwrap();

        manager.disable_routine(&routine.id).unwrap();
        let result = manager.trigger_routine(&routine.id);
        assert!(result.is_err());
    }

    #[test]
    fn test_complete_routine_run() {
        let mut manager = RoutineManager::new();
        let routine = manager
            .create_routine(
                "Test".to_string(),
                "Test routine".to_string(),
                RoutineTrigger::Manual,
                vec![],
            )
            .unwrap();

        let run = manager.trigger_routine(&routine.id).unwrap();
        manager.complete_routine_run(&run.id, true, None).unwrap();

        let runs = manager.get_routine_runs(&routine.id, 10);
        assert_eq!(runs[0].status, "success");
        assert!(runs[0].completed_at.is_some());
    }
}
