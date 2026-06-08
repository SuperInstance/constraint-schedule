//! # constraint-schedule
//!
//! Constraint-satisfaction scheduling for agent task allocation.
//!
//! Treats agent fleet scheduling as a CSP (Constraint Satisfaction Problem).
//! Each agent has capabilities, capacity, and preferences. Tasks have requirements,
//! deadlines, and dependencies. The scheduler finds valid assignments satisfying
//! all hard constraints, then optimizes for soft preferences.
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
pub use constraint::{ConstraintChecker, Constraints};
pub use optimize::ScheduleOptimizer;
pub use schedule::Schedule;
pub use solver::CSPSolver;
pub use task::{TaskSpec, TaskState};
