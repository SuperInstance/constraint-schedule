//! Constraint definitions and checking for scheduling.

use crate::agent::AgentProfile;
use crate::schedule::Schedule;
use crate::task::TaskSpec;

/// Hard and soft constraint configuration.
#[derive(Debug, Clone)]
pub struct Constraints {
    /// Whether to enforce capability matching (agent must have required skills).
    pub capability_match: bool,
    /// Whether to enforce capacity limits.
    pub capacity_limits: bool,
    /// Whether to enforce dependency ordering (deps must finish before task starts).
    pub dependency_ordering: bool,
    /// Whether to enforce deadline constraints.
    pub deadline_enforcement: bool,
    /// Weight for preference alignment in cost function.
    pub preference_weight: f64,
    /// Weight for load balancing in cost function.
    pub load_balance_weight: f64,
    /// Weight for deadline slack in cost function.
    pub deadline_slack_weight: f64,
}

impl Default for Constraints {
    fn default() -> Self {
        Self {
            capability_match: true,
            capacity_limits: true,
            dependency_ordering: true,
            deadline_enforcement: true,
            preference_weight: 1.0,
            load_balance_weight: 1.0,
            deadline_slack_weight: 1.0,
        }
    }
}

impl Constraints {
    /// Create constraints with all hard constraints enabled and default weights.
    pub fn new() -> Self {
        Self::default()
    }

    /// Disable all hard constraints (for testing / relaxed scheduling).
    pub fn relaxed() -> Self {
        Self {
            capability_match: false,
            capacity_limits: false,
            dependency_ordering: false,
            deadline_enforcement: false,
            preference_weight: 1.0,
            load_balance_weight: 1.0,
            deadline_slack_weight: 1.0,
        }
    }
}

/// Result of checking a single constraint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConstraintViolation {
    pub constraint_name: String,
    pub task_id: String,
    pub agent_id: String,
    pub message: String,
}

/// Validates schedules against constraints.
pub struct ConstraintChecker {
    pub constraints: Constraints,
}

impl ConstraintChecker {
    /// Create a new checker with the given constraints.
    pub fn new(constraints: Constraints) -> Self {
        Self { constraints }
    }

    /// Check a single proposed assignment (task → agent) against hard constraints.
    /// Returns a list of violations (empty if valid).
    pub fn check_assignment(
        &self,
        task: &TaskSpec,
        agent: &AgentProfile,
        schedule: &Schedule,
        _tasks: &[TaskSpec],
    ) -> Vec<ConstraintViolation> {
        let mut violations = Vec::new();

        if self.constraints.capability_match
            && !agent.has_capabilities(&task.required_capabilities)
        {
            violations.push(ConstraintViolation {
                constraint_name: "capability_match".into(),
                task_id: task.id.clone(),
                agent_id: agent.id.clone(),
                message: format!(
                    "Agent {} lacks capabilities: {:?}",
                    agent.id, task.required_capabilities
                ),
            });
        }

        if self.constraints.capacity_limits {
            let assigned_count = schedule
                .assignments
                .values()
                .filter(|a| a.agent_id == agent.id)
                .count() as u32;
            if assigned_count >= agent.capacity {
                violations.push(ConstraintViolation {
                    constraint_name: "capacity_limits".into(),
                    task_id: task.id.clone(),
                    agent_id: agent.id.clone(),
                    message: format!(
                        "Agent {} at capacity ({}/{})",
                        agent.id, assigned_count, agent.capacity
                    ),
                });
            }
        }

        if self.constraints.dependency_ordering {
            for dep_id in &task.dependencies {
                if let Some(dep_assignment) = schedule.assignments.get(dep_id) {
                    let dep_end = dep_assignment.start_time + dep_assignment.duration;
                    if let Some(our_assignment) = schedule.assignments.get(&task.id) {
                        if our_assignment.start_time < dep_end {
                            violations.push(ConstraintViolation {
                                constraint_name: "dependency_ordering".into(),
                                task_id: task.id.clone(),
                                agent_id: agent.id.clone(),
                                message: format!(
                                    "Task {} starts at {} but dependency {} ends at {}",
                                    task.id, our_assignment.start_time, dep_id, dep_end
                                ),
                            });
                        }
                    }
                } else {
                    // Dependency not yet scheduled — this is okay during incremental scheduling
                }
            }
        }

        if self.constraints.deadline_enforcement {
            if let Some(assignment) = schedule.assignments.get(&task.id) {
                let end_time = assignment.start_time + assignment.duration;
                if end_time > task.deadline {
                    violations.push(ConstraintViolation {
                        constraint_name: "deadline".into(),
                        task_id: task.id.clone(),
                        agent_id: agent.id.clone(),
                        message: format!(
                            "Task {} ends at {} but deadline is {}",
                            task.id, end_time, task.deadline
                        ),
                    });
                }
            }
        }

        violations
    }

    /// Check an entire schedule for violations.
    pub fn check_schedule(
        &self,
        schedule: &Schedule,
        agents: &[AgentProfile],
        tasks: &[TaskSpec],
    ) -> Vec<ConstraintViolation> {
        let mut violations = Vec::new();

        let agent_map: std::collections::HashMap<&str, &AgentProfile> = agents
            .iter()
            .map(|a| (a.id.as_str(), a))
            .collect();
        let task_map: std::collections::HashMap<&str, &TaskSpec> = tasks
            .iter()
            .map(|t| (t.id.as_str(), t))
            .collect();

        for (task_id, assignment) in &schedule.assignments {
            let task = match task_map.get(task_id.as_str()) {
                Some(t) => t,
                None => continue,
            };
            let agent = match agent_map.get(assignment.agent_id.as_str()) {
                Some(a) => a,
                None => {
                    violations.push(ConstraintViolation {
                        constraint_name: "unknown_agent".into(),
                        task_id: task_id.clone(),
                        agent_id: assignment.agent_id.clone(),
                        message: format!("Unknown agent: {}", assignment.agent_id),
                    });
                    continue;
                }
            };

            if self.constraints.capability_match && !agent.has_capabilities(&task.required_capabilities)
            {
                violations.push(ConstraintViolation {
                    constraint_name: "capability_match".into(),
                    task_id: task_id.clone(),
                    agent_id: agent.id.clone(),
                    message: "Agent lacks required capabilities".to_string(),
                });
            }

            if self.constraints.deadline_enforcement {
                let end = assignment.start_time + assignment.duration;
                if end > task.deadline {
                    violations.push(ConstraintViolation {
                        constraint_name: "deadline".into(),
                        task_id: task_id.clone(),
                        agent_id: agent.id.clone(),
                        message: format!("Misses deadline (ends {} > {})", end, task.deadline),
                    });
                }
            }
        }

        // Capacity check: count assignments per agent
        if self.constraints.capacity_limits {
            let mut counts: std::collections::HashMap<&str, u32> = std::collections::HashMap::new();
            for assignment in schedule.assignments.values() {
                *counts.entry(assignment.agent_id.as_str()).or_insert(0) += 1;
            }
            for agent in agents {
                let count = counts.get(agent.id.as_str()).copied().unwrap_or(0);
                if count > agent.capacity {
                    violations.push(ConstraintViolation {
                        constraint_name: "capacity_limits".into(),
                        task_id: "N/A".into(),
                        agent_id: agent.id.clone(),
                        message: format!(
                            "Agent has {} tasks but capacity is {}",
                            count, agent.capacity
                        ),
                    });
                }
            }
        }

        // Dependency ordering check
        if self.constraints.dependency_ordering {
            for task in tasks {
                if let Some(our_assignment) = schedule.assignments.get(&task.id) {
                    for dep_id in &task.dependencies {
                        if let Some(dep_assignment) = schedule.assignments.get(dep_id) {
                            let dep_end =
                                dep_assignment.start_time + dep_assignment.duration;
                            if our_assignment.start_time < dep_end {
                                violations.push(ConstraintViolation {
                                    constraint_name: "dependency_ordering".into(),
                                    task_id: task.id.clone(),
                                    agent_id: our_assignment.agent_id.clone(),
                                    message: format!(
                                        "Starts {} before dep {} ends {}",
                                        our_assignment.start_time, dep_id, dep_end
                                    ),
                                });
                            }
                        }
                    }
                }
            }
        }

        violations
    }

    /// Compute the soft cost of a schedule (lower = better).
    pub fn compute_cost(
        &self,
        schedule: &Schedule,
        agents: &[AgentProfile],
        tasks: &[TaskSpec],
    ) -> f64 {
        let agent_map: std::collections::HashMap<&str, &AgentProfile> = agents
            .iter()
            .map(|a| (a.id.as_str(), a))
            .collect();
        let task_map: std::collections::HashMap<&str, &TaskSpec> = tasks
            .iter()
            .map(|t| (t.id.as_str(), t))
            .collect();

        let mut cost = 0.0;

        // Preference cost: prefer higher preference scores (negative = good)
        for (task_id, assignment) in &schedule.assignments {
            if let (Some(agent), Some(task)) = (
                agent_map.get(assignment.agent_id.as_str()),
                task_map.get(task_id.as_str()),
            ) {
                let pref = agent.total_preference(&task.required_capabilities);
                // We want to maximize preference, so cost = negative preference
                cost -= self.constraints.preference_weight * pref;
            }
        }

        // Load balance cost: variance in task counts
        let mut counts: std::collections::HashMap<&str, u32> = std::collections::HashMap::new();
        for assignment in schedule.assignments.values() {
            *counts.entry(assignment.agent_id.as_str()).or_insert(0) += 1;
        }
        let avg = counts.values().copied().sum::<u32>() as f64
            / counts.len().max(1) as f64;
        let variance: f64 = counts
            .values()
            .map(|c| (*c as f64 - avg).powi(2))
            .sum();
        cost += self.constraints.load_balance_weight * variance;

        // Deadline slack cost: prefer schedules that finish earlier
        for (task_id, assignment) in &schedule.assignments {
            if let Some(task) = task_map.get(task_id.as_str()) {
                let end = assignment.start_time + assignment.duration;
                if end <= task.deadline {
                    let slack = task.deadline - end;
                    // Lower slack is worse; cost increases with less slack
                    cost += self.constraints.deadline_slack_weight
                        * (1.0 / (slack as f64 + 1.0));
                }
            }
        }

        cost
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schedule::Assignment;

    fn make_agents() -> Vec<AgentProfile> {
        let mut prefs = std::collections::HashMap::new();
        prefs.insert("rust".to_string(), 10.0);
        vec![AgentProfile::new("a1", vec!["rust".into()], 2).with_preferences(prefs)]
    }

    fn make_tasks() -> Vec<TaskSpec> {
        vec![TaskSpec::new("t1", vec!["rust".into()])
            .with_deadline(100)
            .with_duration(10)]
    }

    #[test]
    fn test_default_constraints() {
        let c = Constraints::default();
        assert!(c.capability_match);
        assert!(c.capacity_limits);
        assert!(c.dependency_ordering);
        assert!(c.deadline_enforcement);
    }

    #[test]
    fn test_relaxed_constraints() {
        let c = Constraints::relaxed();
        assert!(!c.capability_match);
        assert!(!c.capacity_limits);
        assert!(!c.dependency_ordering);
        assert!(!c.deadline_enforcement);
    }

    #[test]
    fn test_check_assignment_valid() {
        let checker = ConstraintChecker::new(Constraints::new());
        let agents = make_agents();
        let tasks = make_tasks();
        let schedule = Schedule::new();
        let violations = checker.check_assignment(&tasks[0], &agents[0], &schedule, &tasks);
        assert!(violations.is_empty());
    }

    #[test]
    fn test_check_assignment_capability_mismatch() {
        let checker = ConstraintChecker::new(Constraints::new());
        let agents = make_agents();
        let tasks = vec![TaskSpec::new("t1", vec!["python".into()])];
        let schedule = Schedule::new();
        let violations = checker.check_assignment(&tasks[0], &agents[0], &schedule, &tasks);
        assert!(!violations.is_empty());
        assert_eq!(violations[0].constraint_name, "capability_match");
    }

    #[test]
    fn test_check_assignment_capacity_exceeded() {
        let checker = ConstraintChecker::new(Constraints::new());
        let agents = make_agents(); // capacity 2
        let tasks = make_tasks();
        let mut schedule = Schedule::new();
        // Fill up capacity
        schedule.assignments.insert(
            "t0a".into(),
            Assignment {
                agent_id: "a1".into(),
                start_time: 0,
                duration: 10,
            },
        );
        schedule.assignments.insert(
            "t0b".into(),
            Assignment {
                agent_id: "a1".into(),
                start_time: 0,
                duration: 10,
            },
        );
        let violations = checker.check_assignment(&tasks[0], &agents[0], &schedule, &tasks);
        assert!(violations.iter().any(|v| v.constraint_name == "capacity_limits"));
    }

    #[test]
    fn test_check_schedule_valid() {
        let checker = ConstraintChecker::new(Constraints::new());
        let agents = make_agents();
        let tasks = make_tasks();
        let mut schedule = Schedule::new();
        schedule.assignments.insert(
            "t1".into(),
            Assignment {
                agent_id: "a1".into(),
                start_time: 0,
                duration: 10,
            },
        );
        let violations = checker.check_schedule(&schedule, &agents, &tasks);
        assert!(violations.is_empty());
    }

    #[test]
    fn test_check_schedule_deadline_violation() {
        let checker = ConstraintChecker::new(Constraints::new());
        let agents = make_agents();
        let tasks = vec![TaskSpec::new("t1", vec!["rust".into()])
            .with_deadline(5)
            .with_duration(10)];
        let mut schedule = Schedule::new();
        schedule.assignments.insert(
            "t1".into(),
            Assignment {
                agent_id: "a1".into(),
                start_time: 0,
                duration: 10,
            },
        );
        let violations = checker.check_schedule(&schedule, &agents, &tasks);
        assert!(violations.iter().any(|v| v.constraint_name == "deadline"));
    }

    #[test]
    fn test_compute_cost_prefers_higher_preference() {
        let checker = ConstraintChecker::new(Constraints::new());
        let agents = make_agents();
        let tasks = make_tasks();
        let mut schedule = Schedule::new();
        schedule.assignments.insert(
            "t1".into(),
            Assignment {
                agent_id: "a1".into(),
                start_time: 0,
                duration: 10,
            },
        );
        let cost = checker.compute_cost(&schedule, &agents, &tasks);
        // Should be negative (beneficial) due to high preference
        assert!(cost < 0.0);
    }
}
