//! Task specifications and states for constraint-satisfaction scheduling.

use serde::{Deserialize, Serialize};

/// Current state of a task in the scheduling lifecycle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskState {
    /// Task has not yet been assigned.
    Pending,
    /// Task has been assigned to an agent (contains agent ID).
    Assigned(String),
    /// Task is currently being worked on.
    InProgress,
    /// Task has been completed successfully.
    Completed,
    /// Task has failed.
    Failed,
}

impl TaskState {
    /// Check if the task is assigned (to any agent).
    pub fn is_assigned(&self) -> bool {
        matches!(self, TaskState::Assigned(_))
    }

    /// Check if the task is in a terminal state (completed or failed).
    pub fn is_terminal(&self) -> bool {
        matches!(self, TaskState::Completed | TaskState::Failed)
    }
}

/// Specification of a task to be scheduled.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskSpec {
    /// Unique task identifier.
    pub id: String,
    /// Capabilities required to perform this task.
    pub required_capabilities: Vec<String>,
    /// Deadline as a timestamp (epoch seconds or arbitrary time unit).
    pub deadline: u64,
    /// Estimated duration in the same time unit as deadline.
    pub duration_estimate: u64,
    /// IDs of tasks that must complete before this one can start.
    pub dependencies: Vec<String>,
    /// Priority level (higher = more important).
    pub priority: u32,
    /// Current state of this task.
    pub state: TaskState,
}

impl TaskSpec {
    /// Create a new task specification.
    pub fn new(id: impl Into<String>, required_capabilities: Vec<String>) -> Self {
        Self {
            id: id.into(),
            required_capabilities,
            deadline: 0,
            duration_estimate: 1,
            dependencies: Vec::new(),
            priority: 0,
            state: TaskState::Pending,
        }
    }

    /// Builder: set deadline.
    pub fn with_deadline(mut self, deadline: u64) -> Self {
        self.deadline = deadline;
        self
    }

    /// Builder: set duration estimate.
    pub fn with_duration(mut self, duration: u64) -> Self {
        self.duration_estimate = duration;
        self
    }

    /// Builder: set dependencies.
    pub fn with_dependencies(mut self, deps: Vec<String>) -> Self {
        self.dependencies = deps;
        self
    }

    /// Builder: set priority.
    pub fn with_priority(mut self, priority: u32) -> Self {
        self.priority = priority;
        self
    }

    /// Earliest start time given dependency completion times.
    /// Returns 0 if no dependencies.
    pub fn earliest_start(&self, completion_times: &std::collections::HashMap<String, u64>) -> u64 {
        self.dependencies
            .iter()
            .filter_map(|d| completion_times.get(d))
            .copied()
            .max()
            .unwrap_or(0)
    }

    /// Latest possible start time to meet the deadline.
    pub fn latest_start(&self) -> u64 {
        self.deadline.saturating_sub(self.duration_estimate)
    }

    /// Check if this task has any dependencies.
    pub fn has_dependencies(&self) -> bool {
        !self.dependencies.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_task() {
        let t = TaskSpec::new("t1", vec!["rust".into()]);
        assert_eq!(t.id, "t1");
        assert_eq!(t.required_capabilities, vec!["rust"]);
        assert_eq!(t.deadline, 0);
        assert_eq!(t.duration_estimate, 1);
        assert!(t.dependencies.is_empty());
        assert_eq!(t.priority, 0);
        assert_eq!(t.state, TaskState::Pending);
    }

    #[test]
    fn test_builder_pattern() {
        let t = TaskSpec::new("t1", vec!["rust".into()])
            .with_deadline(100)
            .with_duration(20)
            .with_dependencies(vec!["t0".into()])
            .with_priority(5);
        assert_eq!(t.deadline, 100);
        assert_eq!(t.duration_estimate, 20);
        assert_eq!(t.dependencies, vec!["t0"]);
        assert_eq!(t.priority, 5);
    }

    #[test]
    fn test_earliest_start_no_deps() {
        let t = TaskSpec::new("t1", vec![]);
        let ct = std::collections::HashMap::new();
        assert_eq!(t.earliest_start(&ct), 0);
    }

    #[test]
    fn test_earliest_start_with_deps() {
        let t = TaskSpec::new("t1", vec![]).with_dependencies(vec!["t0".into()]);
        let mut ct = std::collections::HashMap::new();
        ct.insert("t0".to_string(), 50);
        assert_eq!(t.earliest_start(&ct), 50);
    }

    #[test]
    fn test_earliest_start_multiple_deps() {
        let t = TaskSpec::new("t1", vec![]).with_dependencies(vec!["t0".into(), "t0b".into()]);
        let mut ct = std::collections::HashMap::new();
        ct.insert("t0".to_string(), 30);
        ct.insert("t0b".to_string(), 50);
        assert_eq!(t.earliest_start(&ct), 50);
    }

    #[test]
    fn test_latest_start() {
        let t = TaskSpec::new("t1", vec![])
            .with_deadline(100)
            .with_duration(20);
        assert_eq!(t.latest_start(), 80);
    }

    #[test]
    fn test_latest_start_overflow() {
        let t = TaskSpec::new("t1", vec![])
            .with_deadline(5)
            .with_duration(100);
        assert_eq!(t.latest_start(), 0);
    }

    #[test]
    fn test_has_dependencies() {
        let t1 = TaskSpec::new("t1", vec![]);
        assert!(!t1.has_dependencies());
        let t2 = TaskSpec::new("t2", vec![]).with_dependencies(vec!["t1".into()]);
        assert!(t2.has_dependencies());
    }

    #[test]
    fn test_task_state_pending() {
        let s = TaskState::Pending;
        assert!(!s.is_assigned());
        assert!(!s.is_terminal());
    }

    #[test]
    fn test_task_state_assigned() {
        let s = TaskState::Assigned("a1".into());
        assert!(s.is_assigned());
        assert!(!s.is_terminal());
    }

    #[test]
    fn test_task_state_completed() {
        let s = TaskState::Completed;
        assert!(s.is_terminal());
    }

    #[test]
    fn test_task_state_failed() {
        let s = TaskState::Failed;
        assert!(s.is_terminal());
    }

    #[test]
    fn test_serialize_deserialize() {
        let t = TaskSpec::new("t1", vec!["rust".into()])
            .with_deadline(100)
            .with_duration(10);
        let json = serde_json::to_string(&t).unwrap();
        let t2: TaskSpec = serde_json::from_str(&json).unwrap();
        assert_eq!(t.id, t2.id);
        assert_eq!(t.deadline, t2.deadline);
        assert_eq!(t.duration_estimate, t2.duration_estimate);
    }
}
