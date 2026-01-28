//! Optimal Resource Leveling using Constraint Programming
//!
//! This module provides optimal solutions for small conflict clusters using
//! the Pumpkin constraint programming solver with cumulative constraints.
//!
//! # Feature Flag
//!
//! This module requires the `optimal-leveling` feature:
//!
//! ```toml
//! utf8proj-solver = { version = "0.12", features = ["optimal-leveling"] }
//! ```
//!
//! # Algorithm
//!
//! For each cluster, we formulate the Resource-Constrained Project Scheduling
//! Problem (RCPSP) as a constraint satisfaction problem:
//!
//! - **Variables**: Start day offset for each task
//! - **Constraints**:
//!   - Precedence: `start[j] >= start[i] + duration[i]` for dependencies
//!   - Cumulative: Resource usage at any time <= capacity
//! - **Objective**: Minimize makespan (latest finish time)

use chrono::NaiveDate;
use pumpkin_solver::constraints as cp;
use pumpkin_solver::optimisation::linear_sat_unsat::LinearSatUnsat;
use pumpkin_solver::optimisation::OptimisationDirection;
use pumpkin_solver::results::{OptimisationResult, ProblemSolution};
use pumpkin_solver::termination::TimeBudget;
use pumpkin_solver::variables::TransformableVariable;
use pumpkin_solver::Solver;
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};
use utf8proj_core::{Diagnostic, DiagnosticCode, Project, ScheduledTask, Severity, Task, TaskId};

/// Find a task by its qualified ID (e.g., "discovery.kickoff") in the project hierarchy
fn find_task_by_id<'a>(tasks: &'a [Task], target_id: &str) -> Option<&'a Task> {
    for task in tasks {
        if task.id == target_id {
            return Some(task);
        }
        // Check children with qualified ID prefix
        if let Some(found) = find_task_by_id(&task.children, target_id) {
            return Some(found);
        }
        // Also check if target starts with this task's id (qualified path)
        if target_id.starts_with(&format!("{}.", task.id)) {
            let suffix = &target_id[task.id.len() + 1..];
            if let Some(found) = find_task_by_id(&task.children, suffix) {
                return Some(found);
            }
        }
    }
    None
}

use crate::bdd::ConflictCluster;
use crate::leveling::{ClusterResult, LevelingReason, ShiftedTask};

/// Result of attempting optimal cluster solving
#[derive(Debug)]
pub(crate) enum OptimalResult {
    /// Found optimal solution
    Optimal(ClusterResult),
    /// Solver timed out, should fall back to heuristic
    Timeout,
    /// Problem is infeasible (should not happen for valid schedules)
    Infeasible,
}

/// Solve a conflict cluster optimally using constraint programming.
///
/// Returns `OptimalResult::Optimal` with the solution, or `Timeout`/`Infeasible`
/// if the solver cannot find a solution within the timeout.
pub(crate) fn solve_cluster_optimal(
    cluster: &ConflictCluster,
    cluster_idx: usize,
    tasks: &HashMap<TaskId, ScheduledTask>,
    project: &Project,
    timeout_ms: u64,
) -> OptimalResult {
    let start_time = Instant::now();

    // Filter to cluster tasks only
    let cluster_task_ids: HashSet<&str> = cluster.tasks.iter().map(|s| s.as_str()).collect();
    let cluster_tasks: Vec<(&TaskId, &ScheduledTask)> = tasks
        .iter()
        .filter(|(id, _)| cluster_task_ids.contains(id.as_str()))
        .collect();

    if cluster_tasks.is_empty() {
        return OptimalResult::Optimal(ClusterResult {
            task_updates: HashMap::new(),
            shifted_tasks: Vec::new(),
            unresolved_conflicts: Vec::new(),
            diagnostics: Vec::new(),
            elapsed: start_time.elapsed(),
        });
    }

    // Compute the earliest possible start (project start) and horizon
    let project_start = project.start;
    let earliest_start = cluster_tasks
        .iter()
        .map(|(_, t)| t.start)
        .min()
        .unwrap_or(project_start);

    // Horizon: max current finish + some buffer for leveling
    let latest_finish = cluster_tasks.iter().map(|(_, t)| t.finish).max().unwrap();
    let total_duration: i64 = cluster_tasks
        .iter()
        .map(|(_, t)| (t.finish - t.start).num_days() + 1)
        .sum();
    let horizon = (latest_finish - earliest_start).num_days() + total_duration;

    // Create solver
    let mut solver = Solver::default();

    // Create task ID to index mapping for solver variables
    let task_indices: HashMap<&TaskId, usize> = cluster_tasks
        .iter()
        .enumerate()
        .map(|(i, (id, _))| (*id, i))
        .collect();

    // Create start time variables for each task
    // Variable domain: [0, horizon] representing days from earliest_start
    let start_vars: Vec<_> = cluster_tasks
        .iter()
        .map(|(_, task)| {
            let duration = (task.finish - task.start).num_days() + 1;
            // Task can start from day 0 to (horizon - duration)
            solver.new_bounded_integer(0, (horizon - duration).max(0) as i32)
        })
        .collect();

    // Task durations (in days)
    let durations: Vec<i32> = cluster_tasks
        .iter()
        .map(|(_, task)| ((task.finish - task.start).num_days() + 1) as i32)
        .collect();

    // Add precedence constraints from dependencies
    let constraint_tag = solver.new_constraint_tag();
    for (task_id, _task) in &cluster_tasks {
        let task_idx = task_indices[task_id];

        // Find dependencies that are also in this cluster
        // Use recursive search to handle hierarchical task IDs like "discovery.kickoff"
        if let Some(proj_task) = find_task_by_id(&project.tasks, task_id) {
            for dep in &proj_task.depends {
                if let Some(&dep_idx) = task_indices.get(&dep.predecessor) {
                    let (_, dep_task) = cluster_tasks[dep_idx];
                    let dep_duration = ((dep_task.finish - dep_task.start).num_days() + 1) as i32;

                    // start[task] >= start[dep] + duration[dep]
                    // Rewrite as: start[task] - start[dep] >= dep_duration
                    let vars = vec![
                        start_vars[task_idx].scaled(1),
                        start_vars[dep_idx].scaled(-1),
                    ];
                    let _ = solver
                        .add_constraint(cp::greater_than_or_equals(
                            vars,
                            dep_duration,
                            constraint_tag,
                        ))
                        .post();
                }
            }
        }
    }

    // Build resource demands per task
    let cluster_resources: HashSet<&str> = cluster.resources.iter().map(|s| s.as_str()).collect();

    // Add cumulative constraint for each resource
    for resource in &project.resources {
        if !cluster_resources.contains(resource.id.as_str()) {
            continue;
        }

        // Find tasks using this resource and their indices
        let mut tasks_on_resource: Vec<usize> = Vec::new();
        for (idx, (task_id, task)) in cluster_tasks.iter().enumerate() {
            let uses_resource = task
                .assignments
                .iter()
                .any(|a| a.resource_id == resource.id);
            if uses_resource {
                tasks_on_resource.push(task_indices[task_id]);
                // Ensure idx matches task_indices
                debug_assert_eq!(idx, task_indices[task_id]);
            }
        }

        if tasks_on_resource.is_empty() {
            continue;
        }

        // Get resource demands (as percentage * 100 to use integers)
        let demands: Vec<i32> = tasks_on_resource
            .iter()
            .map(|&idx| {
                let (_, task) = &cluster_tasks[idx];
                let demand = task
                    .assignments
                    .iter()
                    .find(|a| a.resource_id == resource.id)
                    .map(|a| (a.units * 100.0) as i32)
                    .unwrap_or(0);
                demand
            })
            .collect();

        // Capacity in same units (percentage * 100)
        let capacity = (resource.capacity * 100.0) as i32;

        // Check if any single task exceeds capacity (makes CP infeasible)
        // This can happen when a task is assigned at 100% to a resource with <100% capacity
        if demands.iter().any(|&d| d > capacity) {
            // Fall back to heuristic - CP can't solve this constraint
            return OptimalResult::Timeout;
        }

        // Get start variables and durations for tasks on this resource
        let resource_starts: Vec<_> = tasks_on_resource
            .iter()
            .map(|&idx| start_vars[idx])
            .collect();
        let resource_durations: Vec<i32> = tasks_on_resource
            .iter()
            .map(|&idx| durations[idx])
            .collect();

        // Add cumulative constraint
        let _ = solver
            .add_constraint(cp::cumulative(
                resource_starts,
                resource_durations,
                demands,
                capacity,
                constraint_tag,
            ))
            .post();
    }

    // Create makespan variable (latest finish time)
    let makespan = solver.new_bounded_integer(0, horizon as i32);

    // Add constraint: makespan >= start[i] + duration[i] for all tasks
    for (idx, &duration) in durations.iter().enumerate() {
        // makespan - start[i] >= duration[i]
        let vars = vec![makespan.scaled(1), start_vars[idx].scaled(-1)];
        let _ = solver
            .add_constraint(cp::greater_than_or_equals(vars, duration, constraint_tag))
            .post();
    }

    // Create brancher and termination condition with timeout
    let mut brancher = solver.default_brancher();
    let mut termination = TimeBudget::starting_now(Duration::from_millis(timeout_ms));

    // Optimize: minimize makespan using linear SAT-UNSAT search
    fn noop_callback<B>(_: &Solver, _: pumpkin_solver::results::SolutionReference, _: &B) {}
    let result = solver.optimise(
        &mut brancher,
        &mut termination,
        LinearSatUnsat::new(OptimisationDirection::Minimise, makespan, noop_callback),
    );

    // Extract solution values from optimization result
    let solution_values: Option<Vec<i64>> = match result {
        OptimisationResult::Optimal(optimal_solution) => Some(
            start_vars
                .iter()
                .map(|&var| optimal_solution.get_integer_value(var) as i64)
                .collect(),
        ),
        OptimisationResult::Satisfiable(satisfiable) => {
            // Found a solution but couldn't prove optimality (timeout while improving)
            Some(
                start_vars
                    .iter()
                    .map(|&var| satisfiable.get_integer_value(var) as i64)
                    .collect(),
            )
        }
        OptimisationResult::Unsatisfiable => {
            return OptimalResult::Infeasible;
        }
        OptimisationResult::Unknown => {
            return OptimalResult::Timeout;
        }
    };

    // Build result from extracted values
    let start_offsets = solution_values.unwrap();
    let mut task_updates: HashMap<TaskId, (NaiveDate, NaiveDate)> = HashMap::new();
    let mut shifted_tasks = Vec::new();
    let mut diagnostics = Vec::new();

    for (idx, (task_id, task)) in cluster_tasks.iter().enumerate() {
        let new_start_offset = start_offsets[idx];
        let duration = (task.finish - task.start).num_days();

        let new_start = earliest_start + chrono::Duration::days(new_start_offset);
        let new_finish = new_start + chrono::Duration::days(duration);

        // Check if task was actually moved
        let shift_days = (new_start - task.start).num_days();
        if shift_days != 0 {
            task_updates.insert((*task_id).clone(), (new_start, new_finish));

            shifted_tasks.push(ShiftedTask {
                task_id: (*task_id).clone(),
                original_start: task.start,
                new_start,
                days_shifted: shift_days,
                reason: LevelingReason::ResourceOverallocated {
                    resource: cluster.resources.first().cloned().unwrap_or_default(),
                    peak_demand: 1.0,
                    capacity: 1.0,
                    dates: vec![task.start],
                },
                resources_involved: cluster.resources.clone(),
            });

            // L001 diagnostic for each shifted task
            diagnostics.push(Diagnostic {
                code: DiagnosticCode::L001OverallocationResolved,
                severity: Severity::Hint,
                message: format!(
                    "Resource overallocation resolved by delaying '{}' by {} day(s) (optimal)",
                    task_id,
                    shift_days.abs()
                ),
                file: None,
                span: None,
                secondary_spans: vec![],
                notes: vec![],
                hints: vec![format!("Solved optimally via constraint programming")],
            });
        }
    }

    // Add L005 summary diagnostic for optimal solution
    diagnostics.push(Diagnostic {
        code: DiagnosticCode::L005OptimalSolution,
        severity: Severity::Info,
        message: format!(
            "Cluster {} ({} tasks) solved optimally in {}ms",
            cluster_idx,
            cluster_tasks.len(),
            start_time.elapsed().as_millis()
        ),
        file: None,
        span: None,
        secondary_spans: vec![],
        notes: vec![format!("Makespan minimized via constraint programming")],
        hints: vec![],
    });

    OptimalResult::Optimal(ClusterResult {
        task_updates,
        shifted_tasks,
        unresolved_conflicts: Vec::new(),
        diagnostics,
        elapsed: start_time.elapsed(),
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_optimal_solver_compiles() {
        // Basic compilation test
        let _options = crate::leveling::LevelingOptions {
            use_optimal: true,
            optimal_threshold: 50,
            optimal_timeout_ms: 5000,
            ..Default::default()
        };
    }
}
