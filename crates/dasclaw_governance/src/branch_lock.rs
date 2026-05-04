//! Branch-lock collision detection.
//!
//! Pure-function port from `claw-code/rust/crates/runtime/src/branch_lock.rs`
//! (#68). Detects when multiple lanes intend to lock the same branch on
//! overlapping module scopes, so the orchestrator can serialize or escalate
//! before the worktrees collide on the filesystem.
//!
//! No actual lock acquisition / release lives here — this module only
//! computes intent collisions on plain data. Locking semantics are owned by
//! the lane scheduler (#71 `lane_events`) once it lands.

use serde::{Deserialize, Serialize};

/// Declared intent of a single lane to take a branch lock on a set of
/// module paths. `worktree` is informational; collisions are computed from
/// `branch` + `modules` only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BranchLockIntent {
    #[serde(rename = "laneId")]
    pub lane_id: String,
    pub branch: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub worktree: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub modules: Vec<String>,
}

/// One detected collision: a `branch` + `module` scope that two or more
/// lanes both intend to lock. `lane_ids` lists the conflicting lanes in
/// pairwise order (left lane first).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BranchLockCollision {
    pub branch: String,
    pub module: String,
    #[serde(rename = "laneIds")]
    pub lane_ids: Vec<String>,
}

/// Pairwise scan over `intents`. Two intents collide when they share a
/// branch and at least one module path overlaps (equality, parent/child
/// scope, or nested scope). Result is sorted + deduped for stable output.
#[must_use]
pub fn detect_branch_lock_collisions(intents: &[BranchLockIntent]) -> Vec<BranchLockCollision> {
    let mut collisions = Vec::new();

    for (index, left) in intents.iter().enumerate() {
        for right in &intents[index + 1..] {
            if left.branch != right.branch {
                continue;
            }
            for module in overlapping_modules(&left.modules, &right.modules) {
                collisions.push(BranchLockCollision {
                    branch: left.branch.clone(),
                    module,
                    lane_ids: vec![left.lane_id.clone(), right.lane_id.clone()],
                });
            }
        }
    }

    collisions.sort_by(|a, b| {
        a.branch
            .cmp(&b.branch)
            .then(a.module.cmp(&b.module))
            .then(a.lane_ids.cmp(&b.lane_ids))
    });
    collisions.dedup();
    collisions
}

fn overlapping_modules(left: &[String], right: &[String]) -> Vec<String> {
    let mut overlaps = Vec::new();
    for left_module in left {
        for right_module in right {
            if modules_overlap(left_module, right_module) {
                overlaps.push(shared_scope(left_module, right_module));
            }
        }
    }
    overlaps.sort();
    overlaps.dedup();
    overlaps
}

fn modules_overlap(left: &str, right: &str) -> bool {
    left == right
        || left.starts_with(&format!("{right}/"))
        || right.starts_with(&format!("{left}/"))
}

fn shared_scope(left: &str, right: &str) -> String {
    if left.starts_with(&format!("{right}/")) || left == right {
        right.to_string()
    } else {
        left.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::{detect_branch_lock_collisions, BranchLockIntent};

    #[test]
    fn req_branch_lock_68_detects_same_branch_same_module_collisions() {
        let collisions = detect_branch_lock_collisions(&[
            BranchLockIntent {
                lane_id: "lane-a".to_string(),
                branch: "feature/lock".to_string(),
                worktree: Some("wt-a".to_string()),
                modules: vec!["runtime/mcp".to_string()],
            },
            BranchLockIntent {
                lane_id: "lane-b".to_string(),
                branch: "feature/lock".to_string(),
                worktree: Some("wt-b".to_string()),
                modules: vec!["runtime/mcp".to_string()],
            },
        ]);

        assert_eq!(collisions.len(), 1);
        assert_eq!(collisions[0].branch, "feature/lock");
        assert_eq!(collisions[0].module, "runtime/mcp");
    }

    #[test]
    fn req_branch_lock_68_detects_nested_module_scope_collisions() {
        let collisions = detect_branch_lock_collisions(&[
            BranchLockIntent {
                lane_id: "lane-a".to_string(),
                branch: "feature/lock".to_string(),
                worktree: None,
                modules: vec!["runtime".to_string()],
            },
            BranchLockIntent {
                lane_id: "lane-b".to_string(),
                branch: "feature/lock".to_string(),
                worktree: None,
                modules: vec!["runtime/mcp".to_string()],
            },
        ]);

        assert_eq!(collisions[0].module, "runtime");
    }

    #[test]
    fn req_branch_lock_68_ignores_different_branches() {
        let collisions = detect_branch_lock_collisions(&[
            BranchLockIntent {
                lane_id: "lane-a".to_string(),
                branch: "feature/a".to_string(),
                worktree: None,
                modules: vec!["runtime/mcp".to_string()],
            },
            BranchLockIntent {
                lane_id: "lane-b".to_string(),
                branch: "feature/b".to_string(),
                worktree: None,
                modules: vec!["runtime/mcp".to_string()],
            },
        ]);

        assert!(collisions.is_empty());
    }
}
