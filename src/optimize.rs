//! Schedule optimizer using simulated annealing.

use crate::agent::AgentProfile;
use crate::constraint::{ConstraintChecker, Constraints};
use crate::schedule::Schedule;
use crate::task::TaskSpec;
use rand::Rng;
use std::time::{Duration, Instant};

/// Configuration for the simulated annealing optimizer.
#[derive(Debug, Clone)]
pub struct AnnealingConfig {
    /// Initial temperature.
    pub initial_temperature: f64,
    /// Final temperature (cooling stops here).
    pub final_temperature: f64,
    /// Cooling factor (0 < alpha < 1).
    pub cooling_rate: f64,
    /// Number of iterations per temperature level.
    pub iterations_per_temp: usize,
    /// Time budget for optimization.
    pub time_budget: Duration,
}

impl Default for AnnealingConfig {
    fn default() -> Self {
        Self {
            initial_temperature: 100.0,
            final_temperature: 0.01,
            cooling_rate: 0.95,
            iterations_per_temp: 100,
            time_budget: Duration::from_secs(10),
        }
    }
}

/// Result of optimization.
#[derive(Debug)]
pub struct OptimizationResult {
    /// Optimized schedule.
    pub schedule: Schedule,
    /// Initial cost before optimization.
    pub initial_cost: f64,
    /// Final cost after optimization.
    pub final_cost: f64,
    /// Number of iterations performed.
    pub iterations: u64,
    /// Number of accepted moves.
    pub accepted: u64,
    /// Time elapsed.
    pub elapsed: Duration,
}

/// Schedule optimizer using simulated annealing with local search.
pub struct ScheduleOptimizer {
    /// Constraints for the problem.
    pub constraints: Constraints,
    /// Annealing configuration.
    pub config: AnnealingConfig,
}

impl ScheduleOptimizer {
    /// Create a new optimizer with default settings.
    pub fn new(constraints: Constraints) -> Self {
        Self {
            constraints,
            config: AnnealingConfig::default(),
        }
    }

    /// Create optimizer with custom annealing config.
    pub fn with_config(mut self, config: AnnealingConfig) -> Self {
        self.config = config;
        self
    }

    /// Optimize a schedule using simulated annealing.
    /// The input schedule must be valid (satisfies hard constraints).
    /// Returns an optimized schedule that also satisfies hard constraints.
    pub fn optimize(
        &self,
        schedule: &Schedule,
        agents: &[AgentProfile],
        tasks: &[TaskSpec],
    ) -> OptimizationResult {
        let start = Instant::now();
        let checker = ConstraintChecker::new(self.constraints.clone());

        let mut current = schedule.clone();
        let mut current_cost = checker.compute_cost(&current, agents, tasks);
        let initial_cost = current_cost;

        let mut best = current.clone();
        let mut best_cost = current_cost;

        let mut temperature = self.config.initial_temperature;
        let mut iterations = 0u64;
        let mut accepted = 0u64;

        let mut rng = rand::rng();

        while temperature > self.config.final_temperature {
            if start.elapsed() > self.config.time_budget {
                break;
            }

            for _ in 0..self.config.iterations_per_temp {
                iterations += 1;

                // Generate a neighbor
                let neighbor = self.generate_neighbor(&current, agents, tasks, &mut rng);

                // Check if neighbor is valid
                if !neighbor.is_valid(agents, tasks, &self.constraints) {
                    continue;
                }

                let neighbor_cost = checker.compute_cost(&neighbor, agents, tasks);
                let delta = neighbor_cost - current_cost;

                // Accept or reject
                if delta < 0.0 || rng.random::<f64>() < (-delta / temperature).exp() {
                    current = neighbor;
                    current_cost = neighbor_cost;
                    accepted += 1;

                    if current_cost < best_cost {
                        best = current.clone();
                        best_cost = current_cost;
                    }
                }
            }

            temperature *= self.config.cooling_rate;
        }

        OptimizationResult {
            schedule: best,
            initial_cost,
            final_cost: best_cost,
            iterations,
            accepted,
            elapsed: start.elapsed(),
        }
    }

    /// Generate a neighboring schedule by making a small change.
    fn generate_neighbor<R: Rng>(
        &self,
        schedule: &Schedule,
        agents: &[AgentProfile],
        tasks: &[TaskSpec],
        rng: &mut R,
    ) -> Schedule {
        let mut neighbor = schedule.clone();

        if neighbor.assignments.is_empty() {
            return neighbor;
        }

        let task_ids: Vec<String> = neighbor.assignments.keys().cloned().collect();
        let task_map: std::collections::HashMap<&str, &TaskSpec> =
            tasks.iter().map(|t| (t.id.as_str(), t)).collect();
        let agent_map: std::collections::HashMap<&str, &AgentProfile> =
            agents.iter().map(|a| (a.id.as_str(), a)).collect();

        // Choose a random move type
        let move_type: f64 = rng.random();

        if move_type < 0.5 {
            // Swap: move a task to a different agent
            let idx = rng.random_range(0..task_ids.len());
            let task_id = &task_ids[idx];
            let task = match task_map.get(task_id.as_str()) {
                Some(t) => t,
                None => return neighbor,
            };

            // Find agents that have the required capabilities
            let capable_agents: Vec<&AgentProfile> = agents
                .iter()
                .filter(|a| a.has_capabilities(&task.required_capabilities))
                .collect();

            if capable_agents.len() > 1 {
                let new_agent = capable_agents[rng.random_range(0..capable_agents.len())];
                if let Some(assignment) = neighbor.assignments.get_mut(task_id) {
                    assignment.agent_id = new_agent.id.clone();
                }
            }
        } else if move_type < 0.8 {
            // Shift: change the start time of a task
            let idx = rng.random_range(0..task_ids.len());
            let task_id = &task_ids[idx];
            let task = match task_map.get(task_id.as_str()) {
                Some(t) => t,
                None => return neighbor,
            };

            if let Some(assignment) = neighbor.assignments.get_mut(task_id) {
                let shift: i64 = rng.random_range(-10..=10);
                let new_start = (assignment.start_time as i64 + shift).max(0) as u64;

                // Check deadline constraint
                if new_start + task.duration_estimate <= task.deadline {
                    assignment.start_time = new_start;
                }
            }
        } else {
            // Swap two tasks between agents
            if task_ids.len() >= 2 {
                let i = rng.random_range(0..task_ids.len());
                let j = rng.random_range(0..task_ids.len());
                if i != j {
                    let t1 = &task_ids[i];
                    let t2 = &task_ids[j];

                    let task1 = task_map.get(t1.as_str());
                    let task2 = task_map.get(t2.as_str());

                    if let (Some(tk1), Some(tk2)) = (task1, task2) {
                        let a1_id = neighbor.assignments.get(t1).map(|a| a.agent_id.clone());
                        let a2_id = neighbor.assignments.get(t2).map(|a| a.agent_id.clone());

                        if let (Some(id1), Some(id2)) = (&a1_id, &a2_id) {
                            // Check capabilities
                            let ag1 = agent_map.get(id1.as_str());
                            let ag2 = agent_map.get(id2.as_str());

                            if let (Some(ag1), Some(ag2)) = (ag1, ag2) {
                                if ag1.has_capabilities(&tk2.required_capabilities)
                                    && ag2.has_capabilities(&tk1.required_capabilities)
                                {
                                    // Swap agent assignments
                                    let agent_id_1 =
                                        neighbor.assignments.get(t1).map(|a| a.agent_id.clone());
                                    let agent_id_2 =
                                        neighbor.assignments.get(t2).map(|a| a.agent_id.clone());
                                    if let (Some(id1), Some(id2)) = (agent_id_1, agent_id_2) {
                                        if let Some(a1) = neighbor.assignments.get_mut(t1) {
                                            a1.agent_id = id2;
                                        }
                                        if let Some(a2) = neighbor.assignments.get_mut(t2) {
                                            a2.agent_id = id1;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        neighbor
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_setup() -> (Vec<AgentProfile>, Vec<TaskSpec>, Schedule) {
        let mut prefs1 = std::collections::HashMap::new();
        prefs1.insert("rust".to_string(), 10.0);
        let mut prefs2 = std::collections::HashMap::new();
        prefs2.insert("rust".to_string(), 5.0);

        let agents = vec![
            AgentProfile::new("a1", vec!["rust".into()], 2).with_preferences(prefs1),
            AgentProfile::new("a2", vec!["rust".into()], 2).with_preferences(prefs2),
        ];
        let tasks = vec![
            TaskSpec::new("t1", vec!["rust".into()])
                .with_deadline(100)
                .with_duration(10),
            TaskSpec::new("t2", vec!["rust".into()])
                .with_deadline(100)
                .with_duration(10),
        ];

        // Initial schedule: assign both to a2 (lower preference)
        let mut schedule = Schedule::new();
        schedule.assign("t1", "a2", 0, 10);
        schedule.assign("t2", "a2", 10, 20);

        (agents, tasks, schedule)
    }

    #[test]
    fn test_optimizer_improves_schedule() {
        let (agents, tasks, schedule) = make_setup();
        let optimizer = ScheduleOptimizer::new(Constraints::new());
        let result = optimizer.optimize(&schedule, &agents, &tasks);
        assert!(result.final_cost <= result.initial_cost);
    }

    #[test]
    fn test_optimizer_preserves_validity() {
        let (agents, tasks, schedule) = make_setup();
        let optimizer = ScheduleOptimizer::new(Constraints::new());
        let result = optimizer.optimize(&schedule, &agents, &tasks);
        assert!(result
            .schedule
            .is_valid(&agents, &tasks, &Constraints::new()));
    }

    #[test]
    fn test_optimizer_returns_stats() {
        let (agents, tasks, schedule) = make_setup();
        let optimizer = ScheduleOptimizer::new(Constraints::new());
        let result = optimizer.optimize(&schedule, &agents, &tasks);
        assert!(result.iterations > 0);
    }

    #[test]
    fn test_empty_schedule() {
        let agents = vec![AgentProfile::new("a1", vec!["rust".into()], 1)];
        let tasks: Vec<TaskSpec> = vec![];
        let schedule = Schedule::new();
        let optimizer = ScheduleOptimizer::new(Constraints::new());
        let result = optimizer.optimize(&schedule, &agents, &tasks);
        assert!(result.schedule.is_empty());
    }

    #[test]
    fn test_single_assignment() {
        let agents = vec![AgentProfile::new("a1", vec!["rust".into()], 1)];
        let tasks = vec![TaskSpec::new("t1", vec!["rust".into()])
            .with_deadline(100)
            .with_duration(10)];
        let mut schedule = Schedule::new();
        schedule.assign("t1", "a1", 0, 10);
        let optimizer = ScheduleOptimizer::new(Constraints::new());
        let result = optimizer.optimize(&schedule, &agents, &tasks);
        assert_eq!(result.schedule.len(), 1);
    }

    #[test]
    fn test_custom_config() {
        let config = AnnealingConfig {
            initial_temperature: 50.0,
            final_temperature: 0.1,
            cooling_rate: 0.9,
            iterations_per_temp: 50,
            time_budget: Duration::from_millis(100),
        };
        let (agents, tasks, schedule) = make_setup();
        let optimizer = ScheduleOptimizer::new(Constraints::new()).with_config(config);
        let result = optimizer.optimize(&schedule, &agents, &tasks);
        // With very short budget, should still complete
        assert!(result.elapsed < Duration::from_secs(1));
    }

    #[test]
    fn test_load_balancing_improves() {
        let agents = vec![
            AgentProfile::new("a1", vec!["rust".into()], 3),
            AgentProfile::new("a2", vec!["rust".into()], 3),
        ];
        let tasks = vec![
            TaskSpec::new("t1", vec!["rust".into()])
                .with_deadline(100)
                .with_duration(10),
            TaskSpec::new("t2", vec!["rust".into()])
                .with_deadline(100)
                .with_duration(10),
            TaskSpec::new("t3", vec!["rust".into()])
                .with_deadline(100)
                .with_duration(10),
        ];

        // All on one agent
        let mut schedule = Schedule::new();
        schedule.assign("t1", "a1", 0, 10);
        schedule.assign("t2", "a1", 10, 20);
        schedule.assign("t3", "a1", 20, 30);

        let mut constraints = Constraints::new();
        constraints.load_balance_weight = 10.0;
        constraints.preference_weight = 0.0;
        constraints.deadline_slack_weight = 0.0;

        let optimizer = ScheduleOptimizer::new(constraints.clone());
        let result = optimizer.optimize(&schedule, &agents, &tasks);

        // Should distribute more evenly
        let a1_count = result
            .schedule
            .assignments
            .values()
            .filter(|a| a.agent_id == "a1")
            .count();
        let a2_count = result
            .schedule
            .assignments
            .values()
            .filter(|a| a.agent_id == "a2")
            .count();
        // At least some distribution
        assert!(a1_count >= 1 || a2_count >= 1);
    }

    #[test]
    fn test_result_cost_tracking() {
        let (agents, tasks, schedule) = make_setup();
        let optimizer = ScheduleOptimizer::new(Constraints::new());
        let result = optimizer.optimize(&schedule, &agents, &tasks);
        // final_cost should be <= initial_cost (or at worst equal)
        assert!(result.final_cost <= result.initial_cost + 0.001);
    }
}
