//! Schedule representation and validation.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A single assignment of a task to an agent at a specific time.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Assignment {
    /// Agent assigned to this task.
    pub agent_id: String,
    /// Start time of the task.
    pub start_time: u64,
    /// Duration of the task.
    pub duration: u64,
}

impl Assignment {
    /// Create a new assignment.
    pub fn new(agent_id: impl Into<String>, start_time: u64, duration: u64) -> Self {
        Self {
            agent_id: agent_id.into(),
            start_time,
            duration,
        }
    }

    /// End time of this assignment.
    pub fn end_time(&self) -> u64 {
        self.start_time + self.duration
    }
}

/// A complete schedule mapping task IDs to assignments.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Schedule {
    /// Map from task ID to assignment.
    pub assignments: HashMap<String, Assignment>,
}

impl Schedule {
    /// Create an empty schedule.
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of assignments in the schedule.
    pub fn len(&self) -> usize {
        self.assignments.len()
    }

    /// Check if the schedule has no assignments.
    pub fn is_empty(&self) -> bool {
        self.assignments.is_empty()
    }

    /// Assign a task to an agent at a given time.
    pub fn assign(
        &mut self,
        task_id: impl Into<String>,
        agent_id: impl Into<String>,
        start_time: u64,
        duration: u64,
    ) {
        self.assignments.insert(
            task_id.into(),
            Assignment::new(agent_id, start_time, duration),
        );
    }

    /// Remove an assignment.
    pub fn unassign(&mut self, task_id: &str) {
        self.assignments.remove(task_id);
    }

    /// Get the assignment for a task.
    pub fn get(&self, task_id: &str) -> Option<&Assignment> {
        self.assignments.get(task_id)
    }

    /// Check validity using a constraint checker.
    pub fn is_valid(
        &self,
        agents: &[crate::agent::AgentProfile],
        tasks: &[crate::task::TaskSpec],
        constraints: &crate::constraint::Constraints,
    ) -> bool {
        let checker = crate::constraint::ConstraintChecker::new(constraints.clone());
        checker.check_schedule(self, agents, tasks).is_empty()
    }

    /// Find conflicts in the schedule.
    pub fn find_conflicts(&self) -> Vec<ScheduleConflict> {
        let mut conflicts = Vec::new();
        let assignments: Vec<(&String, &Assignment)> = self.assignments.iter().collect();

        for i in 0..assignments.len() {
            for j in (i + 1)..assignments.len() {
                let (id_a, a) = assignments[i];
                let (id_b, b) = assignments[j];

                // Same agent, overlapping time
                if a.agent_id == b.agent_id && time_overlaps(a, b) {
                    conflicts.push(ScheduleConflict {
                        task_ids: (id_a.clone(), id_b.clone()),
                        agent_id: a.agent_id.clone(),
                        conflict_type: ConflictType::TimeOverlap,
                        message: format!(
                            "Tasks {} and {} overlap for agent {}",
                            id_a, id_b, a.agent_id
                        ),
                    });
                }
            }
        }

        conflicts
    }

    /// Compute cost (delegates to constraint checker).
    pub fn compute_cost(
        &self,
        agents: &[crate::agent::AgentProfile],
        tasks: &[crate::task::TaskSpec],
        constraints: &crate::constraint::Constraints,
    ) -> f64 {
        let checker = crate::constraint::ConstraintChecker::new(constraints.clone());
        checker.compute_cost(self, agents, tasks)
    }
}

/// A conflict between two assignments.
#[derive(Debug, Clone)]
pub struct ScheduleConflict {
    pub task_ids: (String, String),
    pub agent_id: String,
    pub conflict_type: ConflictType,
    pub message: String,
}

/// Type of schedule conflict.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConflictType {
    /// Two tasks assigned to the same agent overlap in time.
    TimeOverlap,
}

fn time_overlaps(a: &Assignment, b: &Assignment) -> bool {
    a.start_time < b.end_time() && b.start_time < a.end_time()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_schedule() {
        let s = Schedule::new();
        assert!(s.is_empty());
        assert_eq!(s.len(), 0);
    }

    #[test]
    fn test_assign_and_get() {
        let mut s = Schedule::new();
        s.assign("t1", "a1", 0, 10);
        assert!(!s.is_empty());
        assert_eq!(s.len(), 1);
        let a = s.get("t1").unwrap();
        assert_eq!(a.agent_id, "a1");
        assert_eq!(a.start_time, 0);
        assert_eq!(a.duration, 10);
    }

    #[test]
    fn test_unassign() {
        let mut s = Schedule::new();
        s.assign("t1", "a1", 0, 10);
        s.unassign("t1");
        assert!(s.is_empty());
        assert!(s.get("t1").is_none());
    }

    #[test]
    fn test_assignment_end_time() {
        let a = Assignment::new("a1", 10, 20);
        assert_eq!(a.end_time(), 30);
    }

    #[test]
    fn test_no_conflict_different_agents() {
        let mut s = Schedule::new();
        s.assign("t1", "a1", 0, 10);
        s.assign("t2", "a2", 5, 10);
        assert!(s.find_conflicts().is_empty());
    }

    #[test]
    fn test_no_conflict_same_agent_no_overlap() {
        let mut s = Schedule::new();
        s.assign("t1", "a1", 0, 10);
        s.assign("t2", "a1", 10, 10);
        assert!(s.find_conflicts().is_empty());
    }

    #[test]
    fn test_conflict_same_agent_overlap() {
        let mut s = Schedule::new();
        s.assign("t1", "a1", 0, 10);
        s.assign("t2", "a1", 5, 10);
        let conflicts = s.find_conflicts();
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].conflict_type, ConflictType::TimeOverlap);
    }

    #[test]
    fn test_serialize_deserialize() {
        let mut s = Schedule::new();
        s.assign("t1", "a1", 0, 10);
        let json = serde_json::to_string(&s).unwrap();
        let s2: Schedule = serde_json::from_str(&json).unwrap();
        assert_eq!(s.assignments, s2.assignments);
    }
}
