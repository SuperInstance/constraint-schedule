//! Advanced usage: CI/CD pipeline scheduling with dependencies and optimization.
//!
//! Demonstrates a real-world CI/CD scenario where tasks have complex dependency
//! chains, strict deadlines, and agents with different specializations.
//!
//! Run with: cargo run --example advanced

use std::collections::HashMap;
use std::time::Duration;

use constraint_schedule::*;

fn main() {
    println!("=== Advanced: CI/CD Pipeline Scheduling ===\n");

    // Scenario: A CI/CD system with specialized runners
    // Runner 1: Linux builder (rust, docker, test)
    // Runner 2: GPU runner (ml, benchmark)
    // Runner 3: Mac builder (swift, ios, test)
    // Runner 4: Generic runner (docs, lint, deploy)

    let mut linux_prefs = HashMap::new();
    linux_prefs.insert("rust".to_string(), 10.0);
    linux_prefs.insert("docker".to_string(), 8.0);
    linux_prefs.insert("test".to_string(), 5.0);

    let mut gpu_prefs = HashMap::new();
    gpu_prefs.insert("ml".to_string(), 10.0);
    gpu_prefs.insert("benchmark".to_string(), 9.0);

    let mut mac_prefs = HashMap::new();
    mac_prefs.insert("swift".to_string(), 10.0);
    mac_prefs.insert("ios".to_string(), 9.0);
    mac_prefs.insert("test".to_string(), 7.0);

    let mut generic_prefs = HashMap::new();
    generic_prefs.insert("docs".to_string(), 8.0);
    generic_prefs.insert("lint".to_string(), 7.0);
    generic_prefs.insert("deploy".to_string(), 10.0);

    let agents = vec![
        AgentProfile::new("linux-1", vec!["rust".into(), "docker".into(), "test".into()], 2)
            .with_preferences(linux_prefs),
        AgentProfile::new("gpu-1", vec!["ml".into(), "benchmark".into()], 1)
            .with_preferences(gpu_prefs),
        AgentProfile::new("mac-1", vec!["swift".into(), "ios".into(), "test".into()], 2)
            .with_preferences(mac_prefs),
        AgentProfile::new("generic-1", vec!["docs".into(), "lint".into(), "deploy".into()], 3)
            .with_preferences(generic_prefs),
    ];

    // Pipeline stages with dependency chain:
    //
    // lint → build-rust → test-rust → benchmark → deploy
    //                ↘ build-docker ↗            ↗
    // docs ──────────────────────────────────→ deploy
    // build-swift → test-ios ──────────────→ deploy

    let tasks = vec![
        // Stage 0: Independent setup tasks
        TaskSpec::new("lint", vec!["lint".into()])
            .with_deadline(30)
            .with_duration(10)
            .with_priority(3),
        TaskSpec::new("docs", vec!["docs".into()])
            .with_deadline(60)
            .with_duration(20)
            .with_priority(1),
        // Stage 1: Builds
        TaskSpec::new("build-rust", vec!["rust".into()])
            .with_deadline(60)
            .with_duration(25)
            .with_priority(5)
            .with_dependencies(vec!["lint".into()]),
        TaskSpec::new("build-swift", vec!["swift".into()])
            .with_deadline(60)
            .with_duration(30)
            .with_priority(4)
            .with_dependencies(vec!["lint".into()]),
        TaskSpec::new("build-docker", vec!["docker".into()])
            .with_deadline(80)
            .with_duration(20)
            .with_priority(4)
            .with_dependencies(vec!["build-rust".into()]),
        // Stage 2: Tests
        TaskSpec::new("test-rust", vec!["test".into()])
            .with_deadline(90)
            .with_duration(15)
            .with_priority(5)
            .with_dependencies(vec!["build-rust".into()]),
        TaskSpec::new("test-ios", vec!["ios".into()])
            .with_deadline(90)
            .with_duration(20)
            .with_priority(4)
            .with_dependencies(vec!["build-swift".into()]),
        // Stage 3: Analysis
        TaskSpec::new("benchmark", vec!["benchmark".into()])
            .with_deadline(110)
            .with_duration(15)
            .with_priority(3)
            .with_dependencies(vec!["test-rust".into(), "build-docker".into()]),
        // Stage 4: Deploy (everything must pass)
        TaskSpec::new("deploy", vec!["deploy".into()])
            .with_deadline(130)
            .with_duration(10)
            .with_priority(10)
            .with_dependencies(vec![
                "benchmark".into(),
                "test-ios".into(),
                "docs".into(),
            ]),
    ];

    println!("Pipeline: {} tasks across {} agents\n", tasks.len(), agents.len());
    for t in &tasks {
        println!(
            "  {} — needs {:?}, deadline={}, dur={}, deps={:?}, pri={}",
            t.id, t.required_capabilities, t.deadline,
            t.duration_estimate, t.dependencies, t.priority
        );
    }
    println!();

    // Solve with the CSP solver
    println!("--- Solving CSP ---\n");
    let solver = CSPSolver::new().with_time_budget(Duration::from_secs(10));
    let result = solver.solve(&agents, &tasks);

    match &result.schedule {
        Some(schedule) => {
            println!(
                "Found schedule ({} nodes explored, {:.2?})\n",
                result.nodes_explored, result.elapsed
            );

            // Print as timeline
            println!("--- Schedule Timeline ---\n");
            let mut assignments: Vec<_> = schedule.assignments.iter().collect();
            assignments.sort_by_key(|(_, a)| a.start_time);

            for (task_id, assignment) in &assignments {
                let bar_start = assignment.start_time as usize / 5;
                let bar_len = assignment.duration as usize / 5;
                let bar: String = "·".repeat(bar_start) + &"█".repeat(bar_len.max(1));
                println!("{:<15} → {:<10} {}", format!("{}", task_id), assignment.agent_id, bar);
            }
            println!();

            // Validate
            let checker = ConstraintChecker::new(Constraints::new());
            let violations = checker.check_schedule(schedule, &agents, &tasks);
            if violations.is_empty() {
                println!("✓ All constraints satisfied");
            } else {
                println!("✗ Violations:");
                for v in &violations {
                    println!("  - {}: {}", v.constraint_name, v.message);
                }
            }

            let initial_cost = schedule.compute_cost(&agents, &tasks, &Constraints::new());
            println!("Initial cost: {:.3}\n", initial_cost);

            // Optimize
            println!("--- Optimizing with Simulated Annealing ---\n");
            let optimizer =
                ScheduleOptimizer::new(Constraints::new()).with_config(AnnealingConfig {
                    initial_temperature: 100.0,
                    final_temperature: 0.01,
                    cooling_rate: 0.95,
                    iterations_per_temp: 200,
                    time_budget: Duration::from_secs(3),
                });

            let opt_result = optimizer.optimize(schedule, &agents, &tasks);
            println!(
                "Cost: {:.3} → {:.3} ({} iterations, {} accepted, {:.2?})",
                opt_result.initial_cost,
                opt_result.final_cost,
                opt_result.iterations,
                opt_result.accepted,
                opt_result.elapsed
            );

            // Print optimized timeline
            println!("\n--- Optimized Timeline ---\n");
            let mut opt_assignments: Vec<_> = opt_result.schedule.assignments.iter().collect();
            opt_assignments.sort_by_key(|(_, a)| a.start_time);

            for (task_id, assignment) in &opt_assignments {
                let bar_start = assignment.start_time as usize / 5;
                let bar_len = assignment.duration as usize / 5;
                let bar: String = "·".repeat(bar_start) + &"█".repeat(bar_len.max(1));
                println!("{:<15} → {:<10} {}", format!("{}", task_id), assignment.agent_id, bar);
            }

            assert!(opt_result.schedule.is_valid(&agents, &tasks, &Constraints::new()));
            println!("\n✓ Optimized schedule is valid");
        }
        None => {
            println!("No valid schedule found — pipeline is over-constrained!");
            println!("Try relaxing deadlines or adding more agents.");
        }
    }

    println!("\n=== Advanced demo complete! ===");
}
