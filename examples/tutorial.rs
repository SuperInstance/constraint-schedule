//! A guided walkthrough of constraint-satisfaction scheduling.
//!
//! Run with: cargo run --example tutorial

use std::collections::HashMap;
use std::time::Duration;

use constraint_schedule::*;

fn main() {
    println!("=== constraint-schedule Tutorial ===\n");

    // --- Step 1: Define agents ---
    println!("Step 1: Setting up the agent fleet\n");

    let mut alice_prefs = HashMap::new();
    alice_prefs.insert("rust".to_string(), 10.0);
    alice_prefs.insert("wasm".to_string(), 8.0);

    let mut bob_prefs = HashMap::new();
    bob_prefs.insert("python".to_string(), 10.0);
    bob_prefs.insert("ml".to_string(), 9.0);

    let mut carol_prefs = HashMap::new();
    carol_prefs.insert("rust".to_string(), 7.0);
    carol_prefs.insert("python".to_string(), 6.0);

    let agents = vec![
        AgentProfile::new("alice", vec!["rust".into(), "wasm".into()], 2)
            .with_preferences(alice_prefs),
        AgentProfile::new("bob", vec!["python".into(), "ml".into()], 3).with_preferences(bob_prefs),
        AgentProfile::new("carol", vec!["rust".into(), "python".into()], 2)
            .with_preferences(carol_prefs),
    ];

    for a in &agents {
        println!(
            "  {} — skills: {:?}, capacity: {}, remaining: {}",
            a.id,
            a.capabilities,
            a.capacity,
            a.remaining_capacity()
        );
    }
    println!();

    // --- Step 2: Define tasks ---
    println!("Step 2: Defining tasks\n");

    let tasks = vec![
        TaskSpec::new("build-api", vec!["rust".into()])
            .with_deadline(100)
            .with_duration(20)
            .with_priority(5),
        TaskSpec::new("train-model", vec!["ml".into()])
            .with_deadline(80)
            .with_duration(30)
            .with_priority(3),
        TaskSpec::new("deploy-wasm", vec!["wasm".into()])
            .with_deadline(120)
            .with_duration(15)
            .with_dependencies(vec!["build-api".into()]),
        TaskSpec::new("write-tests", vec!["rust".into()])
            .with_deadline(110)
            .with_duration(10)
            .with_dependencies(vec!["build-api".into()]),
        TaskSpec::new("data-pipeline", vec!["python".into()])
            .with_deadline(90)
            .with_duration(25),
    ];

    for t in &tasks {
        println!(
            "  {} — needs {:?}, deadline={}, duration={}, deps={:?}",
            t.id, t.required_capabilities, t.deadline, t.duration_estimate, t.dependencies
        );
    }
    println!();

    // --- Step 3: Solve with CSP ---
    println!("Step 3: CSP Solver\n");

    let solver = CSPSolver::new().with_time_budget(Duration::from_secs(5));
    let result = solver.solve(&agents, &tasks);

    match &result.schedule {
        Some(schedule) => {
            println!("  Found valid schedule!");
            println!("  Nodes explored: {}", result.nodes_explored);
            println!("  Time: {:.2?}", result.elapsed);
            println!();
            for (task_id, assignment) in &schedule.assignments {
                println!(
                    "  {} → {} [t={}, dur={}, end={}]",
                    task_id,
                    assignment.agent_id,
                    assignment.start_time,
                    assignment.duration,
                    assignment.end_time()
                );
            }
        }
        None => {
            println!("  No valid schedule found!");
        }
    }
    println!();

    // --- Step 4: Validate the schedule ---
    println!("Step 4: Validation\n");

    if let Some(ref schedule) = result.schedule {
        let checker = ConstraintChecker::new(Constraints::new());
        let violations = checker.check_schedule(schedule, &agents, &tasks);

        if violations.is_empty() {
            println!("  ✓ Schedule is valid — no constraint violations");
        } else {
            for v in &violations {
                println!("  ✗ {}: {}", v.constraint_name, v.message);
            }
        }
        println!();

        // Check for time conflicts
        let conflicts = schedule.find_conflicts();
        if conflicts.is_empty() {
            println!("  ✓ No time conflicts");
        } else {
            for c in &conflicts {
                println!("  ⚠ {}", c.message);
            }
        }
    }
    println!();

    // --- Step 5: Optimize with simulated annealing ---
    println!("Step 5: Simulated Annealing Optimization\n");

    if let Some(ref schedule) = result.schedule {
        let optimizer = ScheduleOptimizer::new(Constraints::new()).with_config(AnnealingConfig {
            initial_temperature: 100.0,
            final_temperature: 0.1,
            cooling_rate: 0.95,
            iterations_per_temp: 50,
            time_budget: Duration::from_secs(2),
        });

        let opt_result = optimizer.optimize(schedule, &agents, &tasks);
        println!(
            "  Cost: {:.3} → {:.3} ({} iterations, {} accepted)",
            opt_result.initial_cost,
            opt_result.final_cost,
            opt_result.iterations,
            opt_result.accepted
        );
        println!("  Time: {:.2?}", opt_result.elapsed);
        println!();

        println!("  Optimized schedule:");
        for (task_id, assignment) in &opt_result.schedule.assignments {
            println!(
                "  {} → {} [t={}, end={}]",
                task_id,
                assignment.agent_id,
                assignment.start_time,
                assignment.end_time()
            );
        }
        println!();

        // Verify the optimized schedule is still valid
        assert!(opt_result
            .schedule
            .is_valid(&agents, &tasks, &Constraints::new()));
        println!("  ✓ Optimized schedule is valid");
    }
    println!();

    // --- Step 6: Relaxed constraints ---
    println!("Step 6: Relaxed scheduling\n");

    let relaxed_solver = CSPSolver::new().with_constraints(Constraints::relaxed());
    let relaxed_result = relaxed_solver.solve(&agents, &tasks);
    println!(
        "  Relaxed mode found schedule: {}",
        relaxed_result.schedule.is_some()
    );
    println!("  (Relaxed mode ignores capability, capacity, deadline, and dependency constraints)");

    println!("\n=== Tutorial complete! ===");
}
