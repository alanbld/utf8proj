//! Synthetic project generators for benchmarking
//!
//! These generators create Project instances programmatically without parsing,
//! allowing us to benchmark the solver in isolation.

use chrono::NaiveDate;
use utf8proj_core::{Calendar, Duration, Project, Task};

/// Generate a linear chain topology: A -> B -> C -> ... -> N
///
/// This tests:
/// - Recursion depth in topological sort
/// - Stack safety with deep dependency chains
/// - Linear critical path calculation
///
/// Expected project duration: N days (each task is 1 day, all sequential)
pub fn generate_chain(task_count: usize) -> Project {
    let mut project = Project::new("Chain Benchmark");
    project.start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap(); // Monday
    project.calendars = vec![Calendar::default()];

    // Generate tasks in a linear chain
    for i in 0..task_count {
        let task_id = format!("task_{:06}", i);
        let mut task = Task::new(&task_id)
            .name(format!("Task {}", i))
            .duration(Duration::days(1));

        // Each task depends on the previous one
        if i > 0 {
            let prev_id = format!("task_{:06}", i - 1);
            task = task.depends_on(&prev_id);
        }

        project.tasks.push(task);
    }

    project
}

/// Generate a diamond topology: Start -> [N parallel tasks] -> End
///
/// This tests:
/// - Memory allocation for parallel tasks
/// - Multiple predecessors/successors handling
/// - Resource contention (if leveling enabled)
///
/// Expected project duration: 3 days (start:1d + middle:1d + end:1d)
pub fn generate_diamond(parallel_count: usize) -> Project {
    let mut project = Project::new("Diamond Benchmark");
    project.start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
    project.calendars = vec![Calendar::default()];

    // Start task
    let start_task = Task::new("start").name("Start").duration(Duration::days(1));
    project.tasks.push(start_task);

    // Parallel middle tasks
    for i in 0..parallel_count {
        let task_id = format!("middle_{:06}", i);
        let task = Task::new(&task_id)
            .name(format!("Middle {}", i))
            .duration(Duration::days(1))
            .depends_on("start");
        project.tasks.push(task);
    }

    // End task depends on all middle tasks
    let mut end_task = Task::new("end").name("End").duration(Duration::days(1));

    for i in 0..parallel_count {
        let middle_id = format!("middle_{:06}", i);
        end_task = end_task.depends_on(&middle_id);
    }
    project.tasks.push(end_task);

    project
}

/// Generate a random DAG (web) topology with high connectivity
///
/// This tests:
/// - Cycle detection algorithms
/// - Complex topological sort
/// - Multiple paths to same node
///
/// The DAG is constructed by layering:
/// - Divide tasks into layers
/// - Each task in layer N can depend on any task in layers 0..N-1
/// - Use deterministic "random" based on task index for reproducibility
pub fn generate_web(task_count: usize) -> Project {
    let mut project = Project::new("Web Benchmark");
    project.start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
    project.calendars = vec![Calendar::default()];

    if task_count == 0 {
        return project;
    }

    // Divide into layers (sqrt(n) layers with sqrt(n) tasks each)
    let layer_size = (task_count as f64).sqrt().ceil() as usize;
    let num_layers = (task_count + layer_size - 1) / layer_size;

    let mut task_idx = 0;

    for layer in 0..num_layers {
        let tasks_in_layer = std::cmp::min(layer_size, task_count - task_idx);

        for _i in 0..tasks_in_layer {
            let task_id = format!("task_{:06}", task_idx);
            let mut task = Task::new(&task_id)
                .name(format!("Task {}", task_idx))
                .duration(Duration::days(1));

            // Add dependencies to previous layers
            if layer > 0 {
                // Deterministic "random" dependency selection
                // Each task depends on 1-3 tasks from previous layers
                let dep_count = 1 + (task_idx % 3);

                for d in 0..dep_count {
                    // Select dependency from previous layers deterministically
                    let prev_layer_start = (layer - 1) * layer_size;
                    let prev_layer_size = std::cmp::min(layer_size, task_idx - prev_layer_start);

                    if prev_layer_size > 0 {
                        let dep_idx = prev_layer_start + ((task_idx + d * 7) % prev_layer_size);
                        if dep_idx < task_idx {
                            let dep_id = format!("task_{:06}", dep_idx);
                            task = task.depends_on(&dep_id);
                        }
                    }
                }
            }

            project.tasks.push(task);
            task_idx += 1;
        }
    }

    project
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chain_generates_correct_count() {
        let project = generate_chain(100);
        assert_eq!(project.tasks.len(), 100);

        // First task has no dependencies
        assert!(project.tasks[0].depends.is_empty());

        // Last task depends on second-to-last
        assert_eq!(project.tasks[99].depends.len(), 1);
        assert_eq!(project.tasks[99].depends[0].predecessor, "task_000098");
    }

    #[test]
    fn chain_small_scheduling() {
        use utf8proj_core::Scheduler;
        use utf8proj_solver::CpmSolver;

        let project = generate_chain(10);
        let solver = CpmSolver::new();
        let schedule = solver.schedule(&project).unwrap();

        // 10 tasks of 1 day each in sequence = 10 days
        assert_eq!(schedule.project_duration.as_days(), 10.0);

        // All tasks should be critical
        assert_eq!(schedule.critical_path.len(), 10);
    }

    #[test]
    fn diamond_generates_correct_structure() {
        let project = generate_diamond(100);

        // start + 100 middle + end = 102 tasks
        assert_eq!(project.tasks.len(), 102);

        // Find end task
        let end_task = project.tasks.iter().find(|t| t.id == "end").unwrap();
        assert_eq!(end_task.depends.len(), 100);
    }

    #[test]
    fn diamond_small_scheduling() {
        use utf8proj_core::Scheduler;
        use utf8proj_solver::CpmSolver;

        let project = generate_diamond(10);
        let solver = CpmSolver::new();
        let schedule = solver.schedule(&project).unwrap();

        // start:1d + middle:1d (parallel) + end:1d = 3 days
        assert_eq!(schedule.project_duration.as_days(), 3.0);
    }

    #[test]
    fn web_generates_dag() {
        let project = generate_web(100);
        assert_eq!(project.tasks.len(), 100);

        // Verify no circular dependencies by checking task order
        for (i, task) in project.tasks.iter().enumerate() {
            for dep in &task.depends {
                // Extract index from dep id "task_NNNNNN"
                let dep_idx: usize = dep.predecessor[5..].parse().unwrap();
                assert!(dep_idx < i, "Task {} depends on later task {}", i, dep_idx);
            }
        }
    }

    #[test]
    fn web_small_scheduling() {
        use utf8proj_core::Scheduler;
        use utf8proj_solver::CpmSolver;

        let project = generate_web(25);
        let solver = CpmSolver::new();
        let schedule = solver.schedule(&project).unwrap();

        // Should complete successfully
        assert!(schedule.project_duration.as_days() > 0.0);
    }
}
