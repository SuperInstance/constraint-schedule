# constraint-schedule

**Constraint-satisfaction scheduling for agent task allocation.**

You have a fleet of agents, each with specific skills and limited capacity. You have a pile of tasks with deadlines, dependencies, and skill requirements. The combinatorial explosion of possible assignments is astronomical. `constraint-schedule` solves this by treating it as a **Constraint Satisfaction Problem (CSP)** — the same class of problem behind Sudoku solvers and circuit routing.

## The Insight: Fail Early, Search Smart

The naive approach — try every assignment — is O(a^t) where a is agents and t is tasks. Even for 10 agents and 20 tasks, that's 10²⁰ possibilities.

The CSP insight is: **most assignments are obviously wrong, and you can prove it without trying them**. An agent without the "rust" skill can never be assigned a Rust task. A task whose dependency hasn't been scheduled yet is blocked. These constraints prune the search tree *before* you branch, often reducing it by orders of magnitude.

`constraint-schedule` implements three layers of pruning:

1. **AC-3 (Arc Consistency):** Before search even starts, eliminate values from each task's domain that can never participate in any solution.
2. **MRV (Minimum Remaining Values):** Always branch on the task with the fewest valid assignments first — fail fast on the hardest decisions.
3. **Forward Checking:** After each assignment, immediately prune future tasks' domains. If any domain becomes empty, backtrack instantly.

Then, once a valid schedule is found, **simulated annealing** optimizes it for soft preferences — agent affinity, load balancing, and deadline slack.

```
┌──────────────────────────────────────────────────────────┐
│                    CSP Solver Pipeline                    │
├──────────────────────────────────────────────────────────┤
│                                                          │
│  Tasks + Agents                                          │
│       │                                                  │
│       ▼                                                  │
│  ┌─────────┐    ┌──────────┐    ┌──────────────────┐    │
│  │  AC-3   │───▶│   MRV    │───▶│  Backtracking +  │    │
│  │ Arc     │    │  Order   │    │  Forward Check   │    │
│  │ Consist.│    │ (hardest │    │                  │    │
│  └─────────┘    │  first)  │    │  Valid Schedule  │    │
│                 └──────────┘    └───────┬──────────┘    │
│                                         │                │
│                                         ▼                │
│                                ┌──────────────────┐     │
│                                │  Simulated       │     │
│                                │  Annealing       │     │
│                                │  Optimizer       │     │
│                                │                  │     │
│                                │  Optimized       │     │
│                                │  Schedule        │     │
│                                └──────────────────┘     │
└──────────────────────────────────────────────────────────┘
```

## Quick Start

```toml
[dependencies]
constraint-schedule = "0.1"
```

```rust
use constraint_schedule::*;

fn main() {
    // Define agents with capabilities and capacity
    let agents = vec![
        AgentProfile::new("alice", vec!["rust".into(), "wasm".into()], 2),
        AgentProfile::new("bob", vec!["python".into(), "ml".into()], 3),
        AgentProfile::new("carol", vec!["rust".into(), "python".into()], 2),
    ];

    // Define tasks with requirements
    let tasks = vec![
        TaskSpec::new("build-api", vec!["rust".into()])
            .with_deadline(100)
            .with_duration(20),
        TaskSpec::new("train-model", vec!["ml".into()])
            .with_deadline(80)
            .with_duration(30)
            .with_priority(5),
        TaskSpec::new("deploy-wasm", vec!["wasm".into()])
            .with_deadline(120)
            .with_duration(15)
            .with_dependencies(vec!["build-api".into()]),
    ];

    // Solve: find a valid assignment
    let solver = CSPSolver::new();
    let result = solver.solve(&agents, &tasks);

    match result.schedule {
        Some(schedule) => {
            println!("Found schedule in {:.2?}", result.elapsed);
            for (task_id, assignment) in &schedule.assignments {
                println!("  {} → {} (t={}-{})",
                    task_id, assignment.agent_id,
                    assignment.start_time, assignment.end_time());
            }
        }
        None => println!("No valid schedule found"),
    }
}
```

## Tutorial

### Hard Constraints vs. Soft Preferences

`constraint-schedule` separates *hard* constraints (must be satisfied) from *soft* preferences (optimized but not required):

**Hard constraints** (violations make a schedule invalid):
- **Capability match:** Agent must have all skills the task requires
- **Capacity limits:** Agent can't exceed its concurrent task limit
- **Dependency ordering:** A task can't start until all its dependencies finish
- **Deadlines:** A task must finish by its deadline

**Soft preferences** (lower cost = better schedule):
- **Preference alignment:** Match tasks to agents who prefer that work
- **Load balancing:** Distribute work evenly across agents
- **Deadline slack:** Prefer schedules with more breathing room

```rust
let constraints = Constraints::new(); // all hard constraints on, default weights

// Or relax everything (for testing)
let relaxed = Constraints::relaxed();

// Or fine-tune weights
let mut custom = Constraints::new();
custom.preference_weight = 5.0;    // strongly prefer matching skills
custom.load_balance_weight = 2.0;  // moderate load balancing
custom.deadline_slack_weight = 0.5; // slack matters less
```

### Dependencies and Ordering

Tasks can declare dependencies on other tasks. The solver ensures that `t2` doesn't start until `t1` finishes:

```rust
let tasks = vec![
    TaskSpec::new("design", vec!["arch".into()])
        .with_deadline(50).with_duration(10),
    TaskSpec::new("implement", vec!["rust".into()])
        .with_deadline(100).with_duration(30)
        .with_dependencies(vec!["design".into()]),
    TaskSpec::new("test", vec!["rust".into()])
        .with_deadline(150).with_duration(20)
        .with_dependencies(vec!["implement".into()]),
];
```

The MRV heuristic reorders tasks so that those with fewer capable agents are assigned first, and dependencies are naturally handled by the backtracking search.

### Agent Preferences

Give agents preference scores for their skills. The optimizer will prefer assignments that maximize total preference:

```rust
use std::collections::HashMap;

let mut prefs = HashMap::new();
prefs.insert("rust".to_string(), 10.0);
prefs.insert("python".to_string(), 5.0);

let agent = AgentProfile::new("alice", vec!["rust".into(), "python".into()], 3)
    .with_preferences(prefs);
```

### Optimizing with Simulated Annealing

The CSP solver finds *a valid* schedule. The optimizer finds *the best* schedule:

```rust
use std::time::Duration;

let optimizer = ScheduleOptimizer::new(Constraints::new())
    .with_config(AnnealingConfig {
        initial_temperature: 100.0,
        final_temperature: 0.01,
        cooling_rate: 0.95,
        iterations_per_temp: 100,
        time_budget: Duration::from_secs(5),
    });

let result = optimizer.optimize(&initial_schedule, &agents, &tasks);
println!("Cost improved: {:.2} → {:.2} in {} iterations",
    result.initial_cost, result.final_cost, result.iterations);
println!("Accepted {} moves in {:.2?}", result.accepted, result.elapsed);
```

The optimizer makes three types of moves:
1. **Reassign:** Move a task to a different capable agent
2. **Shift:** Change a task's start time (within deadline)
3. **Swap:** Exchange two tasks' agent assignments

Each move is validated against hard constraints before acceptance.

### Finding the Optimal Schedule

For small problems, `solve_optimal` exhaustively searches all valid schedules:

```rust
let result = solver.solve_optimal(&agents, &tasks);
// Returns the lowest-cost valid schedule within the time budget
```

### Schedule Validation

Check any schedule for violations:

```rust
let checker = ConstraintChecker::new(Constraints::new());
let violations = checker.check_schedule(&schedule, &agents, &tasks);
for v in &violations {
    println!("VIOLATION: {} — {}", v.constraint_name, v.message);
}

// Or just check if it's valid
assert!(schedule.is_valid(&agents, &tasks, &Constraints::new()));
```

### Conflict Detection

Find overlapping assignments for the same agent:

```rust
let conflicts = schedule.find_conflicts();
for c in &conflicts {
    println!("Conflict: {} and {} overlap on agent {}",
        c.task_ids.0, c.task_ids.1, c.agent_id);
}
```

## API Reference

### Core Types

| Type | Module | Description |
|------|--------|-------------|
| `AgentProfile` | `agent` | Agent with capabilities, capacity, and preferences |
| `TaskSpec` | `task` | Task with requirements, deadline, and dependencies |
| `TaskState` | `task` | Lifecycle state: Pending → Assigned → InProgress → Completed/Failed |
| `Schedule` | `schedule` | Map from task IDs to assignments |
| `Assignment` | `schedule` | Agent ID + start time + duration |
| `Constraints` | `constraint` | Hard/soft constraint configuration |
| `ConstraintChecker` | `constraint` | Validates schedules against constraints |
| `ConstraintViolation` | `constraint` | A single constraint breach |
| `CSPSolver` | `solver` | Backtracking solver with AC-3 and forward checking |
| `SolverResult` | `solver` | Schedule + stats (nodes explored, time, timeout) |
| `ScheduleOptimizer` | `optimize` | Simulated annealing optimizer |
| `AnnealingConfig` | `optimize` | Temperature schedule and time budget |
| `OptimizationResult` | `optimize` | Optimized schedule + improvement stats |

### Key Methods

```rust
// AgentProfile
AgentProfile::new("id", skills, capacity)
    .with_preferences(prefs)
    .with_load(n);
agent.has_capability("rust")         // skill check
agent.has_capabilities(&skills)       // multi-skill check
agent.remaining_capacity()            // capacity - current_load

// TaskSpec
TaskSpec::new("id", skills)
    .with_deadline(t)
    .with_duration(d)
    .with_dependencies(deps)
    .with_priority(p);
task.earliest_start(&completion_times) // earliest possible start
task.latest_start()                    // latest to meet deadline

// Schedule
schedule.assign("task", "agent", start, duration);
schedule.unassign("task");
schedule.get("task")                   // Option<&Assignment>
schedule.find_conflicts()              // Vec<ScheduleConflict>
schedule.is_valid(&agents, &tasks, &constraints)

// CSPSolver
let solver = CSPSolver::new()
    .with_constraints(constraints)
    .with_time_budget(Duration::from_secs(30));
solver.solve(&agents, &tasks)          // first valid schedule
solver.solve_optimal(&agents, &tasks)  // lowest-cost schedule

// ScheduleOptimizer
let optimizer = ScheduleOptimizer::new(constraints)
    .with_config(config);
optimizer.optimize(&schedule, &agents, &tasks)

// ConstraintChecker
checker.check_assignment(&task, &agent, &schedule, &all_tasks)
checker.check_schedule(&schedule, &agents, &tasks)
checker.compute_cost(&schedule, &agents, &tasks)
```

## Ecosystem Role

`constraint-schedule` is the **resource allocation layer** in the SuperInstance ecosystem:

- **Input:** Tasks generated by user requests or system events; agent profiles describing the fleet
- **Output:** Valid, optimized schedules assigning tasks to agents over time
- **Feeds into:** [`topo-merge`](https://github.com/SuperInstance/topo-merge) for merging agent beliefs about task progress; [`ternary-checkpoint`](https://github.com/SuperInstance/ternary-checkpoint) for checkpointing model weights during distributed training runs

In a SuperInstance deployment, `constraint-schedule` runs continuously: as new tasks arrive and agents complete work, it re-solves the CSP to produce updated schedules that respect all constraints and minimize cost.

## License

MIT
