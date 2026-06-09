//! # constraint-schedule
//!
//! **Constraint-satisfaction scheduling for agent task allocation — CSP solver with AC-3, backtracking, forward checking, and simulated annealing optimization.**
//!
//! When a fleet of agents needs to handle a pile of tasks — each with skill requirements,
//! deadlines, and dependencies — the combinatorial explosion of possible assignments makes
//! brute force impossible. This crate treats scheduling as a Constraint Satisfaction Problem
//! (CSP) and solves it with a layered pruning strategy:
//!
//! 1. **AC-3 (Arc Consistency)** prunes obviously impossible assignments before search
//! 2. **MRV (Minimum Remaining Values)** always branches on the most constrained task first
//! 3. **Backtracking with Forward Checking** detects dead ends early and backtracks instantly
//! 4. **Simulated Annealing** optimizes valid schedules for soft preferences (agent affinity, load balancing, deadline slack)
//!
//! # Quick Example
//!
//! ```
//! use constraint_schedule::*;
//!
//! let agents = vec![
//!     AgentProfile::new("alice", vec!["rust".into()], 2),
//!     AgentProfile::new("bob", vec!["python".into()], 3),
//! ];
//!
//! let tasks = vec![
//!     TaskSpec::new("build-api", vec!["rust".into()])
//!         .with_deadline(100).with_duration(20),
//!     TaskSpec::new("train-model", vec!["python".into()])
//!         .with_deadline(80).with_duration(30),
//! ];
//!
//! let solver = CSPSolver::new();
//! let result = solver.solve(&agents, &tasks);
//! assert!(result.schedule.is_some());
//! ```
//!
//! # Modules
//!
//! - [`agent`] — Agent profiles with capabilities, capacity, and preferences
//! - [`task`] — Task specifications with requirements, deadlines, and dependencies
//! - [`constraint`] — Hard and soft constraint definitions and checking
//! - [`schedule`] — Schedule representation and validation
//! - [`solver`] — CSP solver with backtracking, forward checking, and AC-3
//! - [`optimize`] — Schedule optimizer using simulated annealing

pub mod agent;
pub mod constraint;
pub mod optimize;
pub mod schedule;
pub mod solver;
pub mod task;

pub use agent::AgentProfile;
pub use constraint::{ConstraintChecker, Constraints, ConstraintViolation};
pub use optimize::{ScheduleOptimizer, AnnealingConfig, OptimizationResult};
pub use schedule::{Schedule, Assignment, ScheduleConflict, ConflictType};
pub use solver::{CSPSolver, SolverResult};
pub use task::{TaskSpec, TaskState};
