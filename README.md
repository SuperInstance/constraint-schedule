# constraint-schedule

Constraint-satisfaction scheduling for agent task allocation — a CSP solver with AC-3 arc consistency and simulated annealing optimization, written in pure Rust.

## Overview

When scheduling tasks across an agent fleet, you're solving a **Constraint Satisfaction Problem (CSP)**: find an assignment of tasks → agents × times that satisfies all constraints. This crate treats scheduling as a first-class CSP and solves it with proven AI techniques.

### The Problem

You have:
- **Agents** with capabilities (what they can do), capacity (concurrent task limit), and preferences (what they prefer)
- **Tasks** with required capabilities, deadlines, durations, dependencies, and priorities

You need a **schedule** that:
1. Assigns every task to a capable agent
2. Respects capacity limits (no overloading agents)
3. Honors dependency ordering (deps finish before dependents start)
4. Meets all deadlines
5. *Preferably* aligns with agent preferences, balances load, and maximizes deadline slack

## Architecture

```
┌─────────────┐     ┌─────────────┐
│ AgentProfile │     │  TaskSpec   │
│ capabilities │     │ requirements│
│   capacity   │     │  deadline   │
│ preferences  │     │ dependencies│
└──────┬───────┘     └──────┬──────┘
       │                    │
       ▼                    ▼
┌──────────────────────────────────┐
│        ConstraintChecker         │
│  Hard: capability, capacity,     │
│        dependency, deadline      │
│  Soft: preference, load balance, │
│        deadline slack            │
└──────────────┬───────────────────┘
               │
       ┌───────┴───────┐
       ▼               ▼
┌─────────────┐ ┌─────────────┐
│  CSPSolver  │ │  Schedule   │
│ Backtrack + │ │  Optimizer  │
│  Forward    │ │  Simulated  │
│  Check +    │ │  Annealing  │
│    AC-3     │ │             │
└─────────────┘ └─────────────┘
```

## CSP Solver

The solver uses **backtracking search** enhanced with three pruning techniques:

### Forward Checking

After assigning a task, the solver looks ahead to verify that unassigned tasks still have at least one valid option. If any future task becomes impossible, the solver backtracks immediately — pruning entire subtrees without exploring them.

### AC-3 (Arc Consistency)

Before and during search, AC-3 enforces **arc consistency** across task domains. For every pair of tasks connected by a constraint (e.g., dependency), it removes values from domains that can't possibly lead to a solution. This dramatically reduces the search space.

AC-3 works by maintaining a worklist of constraint arcs `(Xi, Xj)`. For each arc, it checks if removing any value from Xj's domain would leave Xi with no supporting value. If Xi's domain changes, all arcs into Xi are re-added to the worklist.

### Variable Ordering: MRV (Minimum Remaining Values)

Tasks with fewer valid agent/time options are scheduled first. This is the "fail-first" principle — detecting deadlocks early reduces backtracking. A task only one agent can handle is scheduled before a task any agent can handle.

### Value Ordering: LCV (Least Constraining Value)

When assigning a task, the solver tries values that leave the most options for other tasks. Agents with higher capacity are preferred — they constrain the remaining search space less.

### Usage

```rust
use constraint_schedule::*;

let agents = vec![
    AgentProfile::new("alice", vec!["rust".into(), "database".into()], 2),
    AgentProfile::new("bob", vec!["python".into(), "ml".into()], 3),
];

let tasks = vec![
    TaskSpec::new("deploy-api", vec!["rust".into()])
        .with_deadline(100)
        .with_duration(20),
    TaskSpec::new("train-model", vec!["ml".into()])
        .with_deadline(200)
        .with_duration(50)
        .with_dependencies(vec!["deploy-api".into()]),
];

let solver = CSPSolver::new()
    .with_time_budget(std::time::Duration::from_secs(5));

let result = solver.solve(&agents, &tasks);

match result.schedule {
    Some(schedule) => {
        for (task_id, assignment) in &schedule.assignments {
            println!("{} → {} @ t={}", task_id, assignment.agent_id, assignment.start_time);
        }
    }
    None => println!("No valid schedule found"),
}
```

## Schedule Optimizer

Once a valid schedule exists, the **simulated annealing** optimizer improves it through local search:

### How It Works

1. **Neighbor generation**: Make small random changes — swap a task to a different agent, shift start times, or exchange two tasks' agents
2. **Cost evaluation**: Score the schedule on preference alignment, load balance, and deadline slack
3. **Acceptance**: Better schedules always accepted. Worse schedules accepted with probability `e^(-Δ/T)` where T is the current temperature
4. **Cooling**: Temperature decreases geometrically (`T *= α`). Early iterations explore freely; later iterations converge on improvements

### Move Types

| Move | Description |
|------|-------------|
| **Reassign** | Move a task to a different capable agent |
| **Shift** | Change a task's start time |
| **Swap** | Exchange agents between two tasks |

### Usage

```rust
let optimizer = ScheduleOptimizer::new(Constraints::new())
    .with_config(AnnealingConfig {
        initial_temperature: 100.0,
        final_temperature: 0.01,
        cooling_rate: 0.95,
        iterations_per_temp: 100,
        time_budget: std::time::Duration::from_secs(10),
    });

let result = optimizer.optimize(&schedule, &agents, &tasks);
println!("Cost improved: {:.2} → {:.2}", result.initial_cost, result.final_cost);
```

## Constraint Model

### Hard Constraints (must be satisfied)

| Constraint | Description |
|-----------|-------------|
| **Capability match** | Agent must possess all required capabilities |
| **Capacity limits** | Agent's concurrent tasks ≤ capacity |
| **Dependency ordering** | Dependencies finish before dependents start |
| **Deadline enforcement** | Task finishes before its deadline |

### Soft Constraints (optimized via cost function)

| Factor | Weight | Description |
|--------|--------|-------------|
| **Preference alignment** | `preference_weight` | Maximize agent-skill preference scores |
| **Load balancing** | `load_balance_weight` | Minimize variance in task counts across agents |
| **Deadline slack** | `deadline_slack_weight` | Prefer schedules that finish tasks well before deadlines |

## API Reference

### Core Types

- **`AgentProfile`** — Agent with `id`, `capabilities`, `capacity`, `preferences`, `current_load`
- **`TaskSpec`** — Task with `id`, `required_capabilities`, `deadline`, `duration_estimate`, `dependencies`, `priority`, `state`
- **`TaskState`** — `Pending`, `Assigned(agent_id)`, `InProgress`, `Completed`, `Failed`
- **`Schedule`** — Map of `task_id → Assignment` (agent, start_time, duration)
- **`Constraints`** — Hard/soft constraint configuration with adjustable weights

### Solver

- **`CSPSolver::solve()`** — Find first valid schedule
- **`CSPSolver::solve_optimal()`** — Find lowest-cost schedule within time budget
- **`SolverResult`** — Schedule + stats (nodes explored, elapsed, timed_out)

### Optimizer

- **`ScheduleOptimizer::optimize()`** — Improve schedule via simulated annealing
- **`OptimizationResult`** — Optimized schedule + cost tracking + iteration stats

## Installation

```toml
[dependencies]
constraint-schedule = "0.1.0"
```

## License

MIT
