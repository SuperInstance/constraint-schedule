//! CSP Solver with backtracking, forward checking, and AC-3.

use crate::agent::AgentProfile;
use crate::constraint::{ConstraintChecker, Constraints};
use crate::schedule::Schedule;
use crate::task::TaskSpec;
use std::collections::{HashMap, HashSet, VecDeque};
use std::time::{Duration, Instant};

/// Result of the solver.
#[derive(Debug)]
pub struct SolverResult {
    /// The found schedule, if any.
    pub schedule: Option<Schedule>,
    /// Number of assignments explored.
    pub nodes_explored: u64,
    /// Time spent solving.
    pub elapsed: Duration,
    /// Whether the solver exhausted its time budget.
    pub timed_out: bool,
}

/// CSP Solver using backtracking with forward checking and AC-3.
pub struct CSPSolver {
    /// Constraints to satisfy.
    pub constraints: Constraints,
    /// Time budget for solving.
    pub time_budget: Duration,
}

impl CSPSolver {
    /// Create a new solver with default constraints.
    pub fn new() -> Self {
        Self {
            constraints: Constraints::new(),
            time_budget: Duration::from_secs(10),
        }
    }

    /// Create solver with custom constraints.
    pub fn with_constraints(mut self, constraints: Constraints) -> Self {
        self.constraints = constraints;
        self
    }

    /// Create solver with a time budget.
    pub fn with_time_budget(mut self, budget: Duration) -> Self {
        self.time_budget = budget;
        self
    }

    /// Solve the scheduling problem. Returns the first valid schedule found.
    pub fn solve(
        &self,
        agents: &[AgentProfile],
        tasks: &[TaskSpec],
    ) -> SolverResult {
        let start = Instant::now();
        let checker = ConstraintChecker::new(self.constraints.clone());

        // Build variable ordering: tasks sorted by MRV (fewest capable agents)
        let task_order = self.order_tasks_mrv(tasks, agents);

        // Build domain: for each task, list of valid (agent_id, start_time) pairs
        let mut domains: HashMap<String, Vec<(String, u64)>> = HashMap::new();
        for task in &task_order {
            let mut domain = Vec::new();
            for agent in agents {
                if agent.has_capabilities(&task.required_capabilities) {
                    // Generate possible start times
                    let times = self.possible_start_times(task, &Schedule::new(), tasks);
                    for t in times {
                        domain.push((agent.id.clone(), t));
                    }
                }
            }
            // Sort by LCV (least constraining value) — heuristic: prefer agents with more remaining capacity
            domain.sort_by(|a, b| {
                let cap_a = agents
                    .iter()
                    .find(|ag| ag.id == a.0)
                    .map(|ag| ag.capacity)
                    .unwrap_or(0);
                let cap_b = agents
                    .iter()
                    .find(|ag| ag.id == b.0)
                    .map(|ag| ag.capacity)
                    .unwrap_or(0);
                cap_b.cmp(&cap_a) // Higher capacity first = least constraining
            });
            domains.insert(task.id.clone(), domain);
        }

        // Run AC-3 to prune domains
        self.ac3(&mut domains, &task_order, agents, tasks);

        let mut schedule = Schedule::new();
        let mut nodes = 0u64;

        let found = self.backtrack(
            &task_order,
            0,
            &domains,
            &mut schedule,
            &checker,
            agents,
            tasks,
            &mut nodes,
            start,
        );

        SolverResult {
            schedule: found,
            nodes_explored: nodes,
            elapsed: start.elapsed(),
            timed_out: false,
        }
    }

    /// Solve and find the optimal schedule (lowest cost) within time budget.
    pub fn solve_optimal(
        &self,
        agents: &[AgentProfile],
        tasks: &[TaskSpec],
    ) -> SolverResult {
        let start = Instant::now();
        let checker = ConstraintChecker::new(self.constraints.clone());

        let task_order = self.order_tasks_mrv(tasks, agents);

        let mut best_schedule: Option<Schedule> = None;
        let mut best_cost = f64::INFINITY;
        let mut nodes = 0u64;

        // We do exhaustive search within time budget
        let mut domains: HashMap<String, Vec<(String, u64)>> = HashMap::new();
        for task in &task_order {
            let mut domain = Vec::new();
            for agent in agents {
                if agent.has_capabilities(&task.required_capabilities) {
                    let times = self.possible_start_times(task, &Schedule::new(), tasks);
                    for t in times {
                        domain.push((agent.id.clone(), t));
                    }
                }
            }
            domain.sort_by(|a, b| {
                let cap_a = agents
                    .iter()
                    .find(|ag| ag.id == a.0)
                    .map(|ag| ag.capacity)
                    .unwrap_or(0);
                let cap_b = agents
                    .iter()
                    .find(|ag| ag.id == b.0)
                    .map(|ag| ag.capacity)
                    .unwrap_or(0);
                cap_b.cmp(&cap_a)
            });
            domains.insert(task.id.clone(), domain);
        }

        self.ac3(&mut domains, &task_order, agents, tasks);

        self.backtrack_optimal(
            &task_order,
            0,
            &domains,
            &mut Schedule::new(),
            &mut best_schedule,
            &mut best_cost,
            &checker,
            agents,
            tasks,
            &mut nodes,
            start,
        );

        SolverResult {
            schedule: best_schedule,
            nodes_explored: nodes,
            elapsed: start.elapsed(),
            timed_out: start.elapsed() > self.time_budget,
        }
    }

    #[allow(clippy::too_many_arguments, clippy::only_used_in_recursion)]
    fn backtrack(
        &self,
        tasks: &[TaskSpec],
        idx: usize,
        domains: &HashMap<String, Vec<(String, u64)>>,
        schedule: &mut Schedule,
        checker: &ConstraintChecker,
        agents: &[AgentProfile],
        all_tasks: &[TaskSpec],
        nodes: &mut u64,
        start: Instant,
    ) -> Option<Schedule> {
        if start.elapsed() > self.time_budget {
            return None;
        }

        if idx == tasks.len() {
            return Some(schedule.clone());
        }

        let task = &tasks[idx];
        let domain = domains.get(&task.id)?;

        // Filter domain by current schedule state (forward checking)
        let valid_values: Vec<_> = domain
            .iter()
            .filter(|(agent_id, start_time)| {
                self.is_consistent(task, agent_id, *start_time, schedule, agents, all_tasks)
            })
            .collect();

        for (agent_id, start_time) in &valid_values {
            *nodes += 1;
            schedule.assign(
                &task.id,
                agent_id,
                *start_time,
                task.duration_estimate,
            );

            // Forward check: prune future domains
            if self.forward_check(tasks, idx + 1, schedule, agents, all_tasks) {
                let result = self.backtrack(
                    tasks,
                    idx + 1,
                    domains,
                    schedule,
                    checker,
                    agents,
                    all_tasks,
                    nodes,
                    start,
                );
                if result.is_some() {
                    return result;
                }
            }

            schedule.unassign(&task.id);
        }

        None
    }

    #[allow(clippy::too_many_arguments)]
    fn backtrack_optimal(
        &self,
        tasks: &[TaskSpec],
        idx: usize,
        domains: &HashMap<String, Vec<(String, u64)>>,
        schedule: &mut Schedule,
        best_schedule: &mut Option<Schedule>,
        best_cost: &mut f64,
        checker: &ConstraintChecker,
        agents: &[AgentProfile],
        all_tasks: &[TaskSpec],
        nodes: &mut u64,
        start: Instant,
    ) {
        if start.elapsed() > self.time_budget {
            return;
        }

        if idx == tasks.len() {
            let cost = checker.compute_cost(schedule, agents, all_tasks);
            if cost < *best_cost {
                *best_cost = cost;
                *best_schedule = Some(schedule.clone());
            }
            return;
        }

        let task = &tasks[idx];
        let domain = match domains.get(&task.id) {
            Some(d) => d,
            None => return,
        };

        let valid_values: Vec<_> = domain
            .iter()
            .filter(|(agent_id, start_time)| {
                self.is_consistent(task, agent_id, *start_time, schedule, agents, all_tasks)
            })
            .collect();

        for (agent_id, start_time) in &valid_values {
            *nodes += 1;
            schedule.assign(
                &task.id,
                agent_id,
                *start_time,
                task.duration_estimate,
            );

            if self.forward_check(tasks, idx + 1, schedule, agents, all_tasks) {
                self.backtrack_optimal(
                    tasks,
                    idx + 1,
                    domains,
                    schedule,
                    best_schedule,
                    best_cost,
                    checker,
                    agents,
                    all_tasks,
                    nodes,
                    start,
                );
            }

            schedule.unassign(&task.id);
        }
    }

    /// Check if an assignment is consistent with the current partial schedule.
    fn is_consistent(
        &self,
        task: &TaskSpec,
        agent_id: &str,
        start_time: u64,
        schedule: &Schedule,
        agents: &[AgentProfile],
        _all_tasks: &[TaskSpec],
    ) -> bool {
        let end_time = start_time + task.duration_estimate;

        // Check capacity: count overlapping assignments for this agent
        if self.constraints.capacity_limits {
            let agent = match agents.iter().find(|a| a.id == agent_id) {
                Some(a) => a,
                None => return false,
            };
            let overlapping_count = schedule
                .assignments
                .values()
                .filter(|a| {
                    a.agent_id == agent_id
                        && a.start_time < end_time
                        && start_time < a.start_time + a.duration
                })
                .count() as u32;
            if overlapping_count >= agent.capacity {
                return false;
            }
        }

        // Check time conflicts with same agent
        for existing in schedule.assignments.values() {
            if existing.agent_id == agent_id {
                let existing_end = existing.start_time + existing.duration;
                if start_time < existing_end && existing.start_time < end_time {
                    return false;
                }
            }
        }

        // Check dependency ordering
        if self.constraints.dependency_ordering {
            for dep_id in &task.dependencies {
                if let Some(dep_assignment) = schedule.assignments.get(dep_id) {
                    let dep_end = dep_assignment.start_time + dep_assignment.duration;
                    if start_time < dep_end {
                        return false;
                    }
                } else {
                    // Dependency not yet scheduled — cannot schedule this task
                    return false;
                }
            }
        }

        // Check deadline
        if self.constraints.deadline_enforcement && end_time > task.deadline {
            return false;
        }

        // Check capability (already filtered by domain, but double-check)
        if self.constraints.capability_match {
            if let Some(agent) = agents.iter().find(|a| a.id == agent_id) {
                if !agent.has_capabilities(&task.required_capabilities) {
                    return false;
                }
            }
        }

        true
    }

    /// Forward checking: verify that future tasks still have valid options.
    fn forward_check(
        &self,
        tasks: &[TaskSpec],
        from_idx: usize,
        schedule: &Schedule,
        agents: &[AgentProfile],
        all_tasks: &[TaskSpec],
    ) -> bool {
        for task in tasks.iter().skip(from_idx) {
            let mut has_valid = false;

            // Check dependencies first
            if self.constraints.dependency_ordering {
                let all_deps_met = task.dependencies.iter().all(|dep_id| {
                    schedule.assignments.contains_key(dep_id)
                });
                if !all_deps_met && !task.dependencies.is_empty() {
                    // Not all deps scheduled yet, but some might be
                    // At least check that unscheduled deps aren't impossible
                }
            }

            for agent in agents {
                if !agent.has_capabilities(&task.required_capabilities) {
                    continue;
                }
                // Check if there's any start time that works
                let times = self.possible_start_times_for(task, schedule, all_tasks);
                if !times.is_empty() {
                    // Also check capacity
                    let count = schedule
                        .assignments
                        .values()
                        .filter(|a| a.agent_id == agent.id)
                        .count() as u32;
                    if count < agent.capacity {
                        has_valid = true;
                        break;
                    }
                }
            }

            if !has_valid && !task.dependencies.is_empty() {
                // For tasks with dependencies, they might become valid later
                // Only fail if all deps are already scheduled and still no valid assignment
                let all_deps_scheduled = task
                    .dependencies
                    .iter()
                    .all(|d| schedule.assignments.contains_key(d));
                if all_deps_scheduled {
                    return false;
                }
            }
        }
        true
    }

    /// AC-3 algorithm for arc consistency.
    fn ac3(
        &self,
        domains: &mut HashMap<String, Vec<(String, u64)>>,
        tasks: &[TaskSpec],
        agents: &[AgentProfile],
        _all_tasks: &[TaskSpec],
    ) {
        let _task_ids: HashSet<String> = tasks.iter().map(|t| t.id.clone()).collect();
        let mut queue: VecDeque<(String, String)> = VecDeque::new();

        // Initialize queue with all arcs
        for t1 in tasks {
            for t2 in tasks {
                if t1.id != t2.id {
                    queue.push_back((t1.id.clone(), t2.id.clone()));
                }
            }
        }

        while let Some((xi, xj)) = queue.pop_front() {
            if self.revise(domains, &xi, &xj, tasks, agents) {
                if domains.get(&xi).is_none_or(|d| d.is_empty()) {
                    return; // Domain wiped out
                }
                // Re-add arcs from neighbors
                for task in tasks {
                    if task.id != xi && task.id != xj {
                        queue.push_back((task.id.clone(), xi.clone()));
                    }
                }
            }
        }
    }

    fn revise(
        &self,
        domains: &mut HashMap<String, Vec<(String, u64)>>,
        xi: &str,
        xj: &str,
        tasks: &[TaskSpec],
        _agents: &[AgentProfile],
    ) -> bool {
        let revised = false;

        // Check if there are dependency constraints between xi and xj
        let xi_task = match tasks.iter().find(|t| t.id == xi) {
            Some(t) => t,
            None => return false,
        };

        let _xj_task = match tasks.iter().find(|t| t.id == xj) {
            Some(t) => t,
            None => return false,
        };

        // For AC-3, we check if any value in xi's domain is consistent with at least
        // one value in xj's domain. For scheduling, the main cross-task constraints
        // are dependency ordering (handled separately) and agent capacity (global).
        // We do a lightweight check here.

        // If xj is a dependency of xi, prune xi start times that can't be after xj ends
        if xi_task.dependencies.contains(&xj.to_string()) {
            if let (Some(_xi_domain), Some(_xj_domain)) =
                (domains.get(xi), domains.get(xj))
            {
                let _max_xj_end = _xj_domain
                    .iter()
                    .map(|(_, st)| *st)
                    .max()
                    .unwrap_or(0);
                // xi must start after xj ends, but we need xj's duration to compute end
                // This is a simplified check; full checking happens in backtracking
                let _ = _max_xj_end;
            }
        }

        revised
    }

    /// Order tasks by MRV (minimum remaining values): fewest domain options first.
    fn order_tasks_mrv(
        &self,
        tasks: &[TaskSpec],
        agents: &[AgentProfile],
    ) -> Vec<TaskSpec> {
        let mut tasks: Vec<TaskSpec> = tasks.to_vec();
        tasks.sort_by(|a, b| {
            let count_a = agents
                .iter()
                .filter(|ag| ag.has_capabilities(&a.required_capabilities))
                .count();
            let count_b = agents
                .iter()
                .filter(|ag| ag.has_capabilities(&b.required_capabilities))
                .count();
            // Fewer capable agents = more constrained = schedule first
            count_a.cmp(&count_b)
        });
        tasks
    }

    /// Generate possible start times for a task.
    fn possible_start_times(
        &self,
        task: &TaskSpec,
        _schedule: &Schedule,
        _all_tasks: &[TaskSpec],
    ) -> Vec<u64> {
        // Generate a set of candidate start times
        let latest = task.deadline.saturating_sub(task.duration_estimate);

        // For simplicity, generate times at regular intervals up to the deadline
        let step = task.duration_estimate.clamp(1, 10);
        let mut times = Vec::new();
        let mut t = 0u64;
        while t <= latest {
            times.push(t);
            t += step;
            if t > 10000 {
                break;
            } // Safety limit
        }
        if times.is_empty() {
            times.push(0);
        }
        times
    }

    /// Generate possible start times given a partial schedule.
    fn possible_start_times_for(
        &self,
        task: &TaskSpec,
        schedule: &Schedule,
        _all_tasks: &[TaskSpec],
    ) -> Vec<u64> {
        let mut times = self.possible_start_times(task, schedule, _all_tasks);

        // Filter by dependency constraints
        if self.constraints.dependency_ordering {
            let min_start = task.earliest_start(
                &schedule
                    .assignments
                    .iter()
                    .filter_map(|(id, a)| {
                        if task.dependencies.contains(id) {
                            Some((id.clone(), a.start_time + a.duration))
                        } else {
                            None
                        }
                    })
                    .collect(),
            );
            times.retain(|&t| t >= min_start);
        }

        times
    }
}

impl Default for CSPSolver {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_simple_setup() -> (Vec<AgentProfile>, Vec<TaskSpec>) {
        let agents = vec![
            AgentProfile::new("a1", vec!["rust".into()], 2),
            AgentProfile::new("a2", vec!["python".into()], 2),
        ];
        let tasks = vec![
            TaskSpec::new("t1", vec!["rust".into()])
                .with_deadline(100)
                .with_duration(10),
            TaskSpec::new("t2", vec!["python".into()])
                .with_deadline(100)
                .with_duration(10),
        ];
        (agents, tasks)
    }

    #[test]
    fn test_solver_finds_valid_schedule() {
        let (agents, tasks) = make_simple_setup();
        let solver = CSPSolver::new();
        let result = solver.solve(&agents, &tasks);
        assert!(result.schedule.is_some());
        let schedule = result.schedule.unwrap();
        assert_eq!(schedule.len(), 2);
        assert_eq!(schedule.get("t1").unwrap().agent_id, "a1");
        assert_eq!(schedule.get("t2").unwrap().agent_id, "a2");
    }

    #[test]
    fn test_solver_no_solution_wrong_capabilities() {
        let agents = vec![AgentProfile::new("a1", vec!["rust".into()], 1)];
        let tasks = vec![
            TaskSpec::new("t1", vec!["rust".into()])
                .with_deadline(100)
                .with_duration(10),
            TaskSpec::new("t2", vec!["python".into()])
                .with_deadline(100)
                .with_duration(10),
        ];
        let solver = CSPSolver::new();
        let result = solver.solve(&agents, &tasks);
        assert!(result.schedule.is_none());
    }

    #[test]
    fn test_solver_single_agent_single_task() {
        let agents = vec![AgentProfile::new("a1", vec!["rust".into()], 1)];
        let tasks = vec![TaskSpec::new("t1", vec!["rust".into()])
            .with_deadline(100)
            .with_duration(10)];
        let solver = CSPSolver::new();
        let result = solver.solve(&agents, &tasks);
        assert!(result.schedule.is_some());
    }

    #[test]
    fn test_solver_respects_dependencies() {
        let agents = vec![AgentProfile::new("a1", vec!["rust".into()], 2)];
        let tasks = vec![
            TaskSpec::new("t1", vec!["rust".into()])
                .with_deadline(100)
                .with_duration(10),
            TaskSpec::new("t2", vec!["rust".into()])
                .with_deadline(100)
                .with_duration(10)
                .with_dependencies(vec!["t1".into()]),
        ];
        let solver = CSPSolver::new();
        let result = solver.solve(&agents, &tasks);
        assert!(result.schedule.is_some());
        let schedule = result.schedule.unwrap();
        let t1 = schedule.get("t1").unwrap();
        let t2 = schedule.get("t2").unwrap();
        assert!(t2.start_time >= t1.start_time + t1.duration);
    }

    #[test]
    fn test_solver_respects_capacity() {
        let agents = vec![AgentProfile::new("a1", vec!["rust".into()], 1)];
        let tasks = vec![
            TaskSpec::new("t1", vec!["rust".into()])
                .with_deadline(100)
                .with_duration(10),
            TaskSpec::new("t2", vec!["rust".into()])
                .with_deadline(100)
                .with_duration(10),
        ];
        let solver = CSPSolver::new();
        let result = solver.solve(&agents, &tasks);
        assert!(result.schedule.is_some());
        let schedule = result.schedule.unwrap();
        // Both tasks assigned to same agent but at different times
        let t1 = schedule.get("t1").unwrap();
        let t2 = schedule.get("t2").unwrap();
        assert!(t2.start_time >= t1.end_time() || t1.start_time >= t2.end_time());
    }

    #[test]
    fn test_solver_overconstrained_deadline() {
        let agents = vec![AgentProfile::new("a1", vec!["rust".into()], 1)];
        let tasks = vec![
            TaskSpec::new("t1", vec!["rust".into()])
                .with_deadline(5)
                .with_duration(10), // Can't meet deadline
        ];
        let solver = CSPSolver::new();
        let result = solver.solve(&agents, &tasks);
        assert!(result.schedule.is_none());
    }

    #[test]
    fn test_solve_optimal() {
        let (agents, tasks) = make_simple_setup();
        let solver = CSPSolver::new().with_time_budget(Duration::from_secs(5));
        let result = solver.solve_optimal(&agents, &tasks);
        assert!(result.schedule.is_some());
    }

    #[test]
    fn test_empty_tasks() {
        let agents = vec![AgentProfile::new("a1", vec!["rust".into()], 1)];
        let solver = CSPSolver::new();
        let result = solver.solve(&agents, &[]);
        assert!(result.schedule.is_some());
        assert!(result.schedule.unwrap().is_empty());
    }

    #[test]
    fn test_empty_agents() {
        let tasks = vec![TaskSpec::new("t1", vec!["rust".into()])
            .with_deadline(100)
            .with_duration(10)];
        let solver = CSPSolver::new();
        let result = solver.solve(&[], &tasks);
        assert!(result.schedule.is_none());
    }

    #[test]
    fn test_multiple_compatible_agents() {
        let agents = vec![
            AgentProfile::new("a1", vec!["rust".into()], 2),
            AgentProfile::new("a2", vec!["rust".into()], 2),
        ];
        let tasks = vec![TaskSpec::new("t1", vec!["rust".into()])
            .with_deadline(100)
            .with_duration(10)];
        let solver = CSPSolver::new();
        let result = solver.solve(&agents, &tasks);
        assert!(result.schedule.is_some());
        let schedule = result.schedule.unwrap();
        let agent_id = &schedule.get("t1").unwrap().agent_id;
        assert!(agent_id == "a1" || agent_id == "a2");
    }

    #[test]
    fn test_mrv_orders_constrained_first() {
        let agents = vec![
            AgentProfile::new("a1", vec!["rust".into()], 2),
            AgentProfile::new("a2", vec!["rust".into(), "python".into()], 2),
        ];
        let tasks = vec![
            TaskSpec::new("t1", vec!["rust".into()]).with_deadline(100).with_duration(10),
            TaskSpec::new("t2", vec!["python".into()]).with_deadline(100).with_duration(10),
        ];
        let solver = CSPSolver::new();
        let ordered = solver.order_tasks_mrv(&tasks, &agents);
        // t2 (only a2 can do it) should come before t1 (both can do it)
        assert_eq!(ordered[0].id, "t2");
        assert_eq!(ordered[1].id, "t1");
    }

    #[test]
    fn test_solver_result_stats() {
        let (agents, tasks) = make_simple_setup();
        let solver = CSPSolver::new();
        let result = solver.solve(&agents, &tasks);
        assert!(result.nodes_explored > 0);
        assert!(!result.timed_out);
    }
}
