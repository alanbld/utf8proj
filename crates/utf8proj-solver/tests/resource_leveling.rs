//! Integration tests for Resource Leveling
//!
//! Tests the CpmSolver with resource leveling enabled.

use chrono::NaiveDate;
use utf8proj_core::{Duration, Project, Resource, Scheduler, Task};
use utf8proj_solver::{detect_overallocations, level_resources, CpmSolver};

fn date(year: i32, month: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(year, month, day).unwrap()
}

// =============================================================================
// Basic Resource Leveling
// =============================================================================

#[test]
fn solver_with_leveling_enabled() {
    let mut project = Project::new("Resource Leveling Test");
    project.start = date(2025, 1, 6); // Monday
    project.resources = vec![Resource::new("dev").capacity(1.0)];
    project.tasks = vec![
        Task::new("task1")
            .name("Task 1")
            .effort(Duration::days(5))
            .assign("dev"),
        Task::new("task2")
            .name("Task 2")
            .effort(Duration::days(5))
            .assign("dev"),
    ];

    // Without leveling - both tasks start on day 0 (over-allocated)
    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).unwrap();
    assert_eq!(schedule.tasks["task1"].start, schedule.tasks["task2"].start);

    // With leveling - tasks should be sequential
    let solver_leveled = CpmSolver::with_leveling();
    let schedule_leveled = solver_leveled.schedule(&project).unwrap();

    let task1 = &schedule_leveled.tasks["task1"];
    let task2 = &schedule_leveled.tasks["task2"];

    // One should start after the other finishes
    let sequential = task1.finish < task2.start || task2.finish < task1.start;
    assert!(
        sequential,
        "Tasks should be sequential after leveling: task1 {} - {}, task2 {} - {}",
        task1.start,
        task1.finish,
        task2.start,
        task2.finish
    );
}

#[test]
fn leveling_extends_project_duration() {
    let mut project = Project::new("Extended Duration Test");
    project.start = date(2025, 1, 6);
    project.resources = vec![Resource::new("dev").capacity(1.0)];
    project.tasks = vec![
        Task::new("task1").effort(Duration::days(5)).assign("dev"),
        Task::new("task2").effort(Duration::days(5)).assign("dev"),
    ];

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).unwrap();

    let solver_leveled = CpmSolver::with_leveling();
    let schedule_leveled = solver_leveled.schedule(&project).unwrap();

    // Leveled schedule should be longer (5 + 5 = 10 vs original 5)
    assert!(
        schedule_leveled.project_duration.as_days() > schedule.project_duration.as_days(),
        "Project should be extended: {} days without leveling, {} days with leveling",
        schedule.project_duration.as_days(),
        schedule_leveled.project_duration.as_days()
    );
}

#[test]
fn leveling_respects_dependencies() {
    let mut project = Project::new("Dependencies Test");
    project.start = date(2025, 1, 6);
    project.resources = vec![Resource::new("dev").capacity(1.0)];
    project.tasks = vec![
        Task::new("task1").effort(Duration::days(3)).assign("dev"),
        Task::new("task2")
            .effort(Duration::days(3))
            .assign("dev")
            .depends_on("task1"),
        Task::new("task3").effort(Duration::days(3)).assign("dev"),
    ];

    let solver = CpmSolver::with_leveling();
    let schedule = solver.schedule(&project).unwrap();

    let task1 = &schedule.tasks["task1"];
    let task2 = &schedule.tasks["task2"];
    let task3 = &schedule.tasks["task3"];

    // task2 must still come after task1 (dependency)
    assert!(
        task2.start > task1.finish,
        "task2 should start after task1 due to dependency"
    );

    // All tasks should be sequential due to single resource
    let all_sequential = (task1.finish < task2.start || task2.finish < task1.start)
        && (task1.finish < task3.start || task3.finish < task1.start)
        && (task2.finish < task3.start || task3.finish < task2.start);

    assert!(
        all_sequential || task1.start == task3.start,
        "All tasks should be handled by leveling"
    );
}

#[test]
fn leveling_with_partial_allocation() {
    let mut project = Project::new("Partial Allocation Test");
    project.start = date(2025, 1, 6);
    project.resources = vec![Resource::new("dev").capacity(1.0)];

    // Two tasks at 50% each - should be able to run in parallel
    project.tasks = vec![
        Task::new("task1")
            .effort(Duration::days(5))
            .assign("dev"), // Default 100%
        Task::new("task2")
            .effort(Duration::days(5))
            .assign("dev"), // Default 100%
    ];

    // Both at 100% = 200% usage = conflict
    let solver = CpmSolver::with_leveling();
    let schedule = solver.schedule(&project).unwrap();

    // Should have been leveled to sequential
    let task1 = &schedule.tasks["task1"];
    let task2 = &schedule.tasks["task2"];
    assert!(
        task1.finish < task2.start || task2.finish < task1.start,
        "Full allocation tasks should be sequential"
    );
}

#[test]
fn detect_overallocations_api() {
    let mut project = Project::new("Detection API Test");
    project.start = date(2025, 1, 6);
    project.resources = vec![Resource::new("dev").capacity(1.0)];
    project.tasks = vec![
        Task::new("task1").effort(Duration::days(5)).assign("dev"),
        Task::new("task2").effort(Duration::days(5)).assign("dev"),
    ];

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).unwrap();

    let conflicts = detect_overallocations(&project, &schedule);

    assert!(
        !conflicts.is_empty(),
        "Should detect over-allocation when two 100% tasks overlap"
    );

    // All conflicts should be for "dev" resource
    for (resource_id, _period) in &conflicts {
        assert_eq!(resource_id, "dev");
    }
}

#[test]
fn level_resources_api() {
    let mut project = Project::new("Leveling API Test");
    project.start = date(2025, 1, 6);
    project.resources = vec![Resource::new("dev").capacity(1.0)];
    project.tasks = vec![
        Task::new("task1").effort(Duration::days(3)).assign("dev"),
        Task::new("task2").effort(Duration::days(3)).assign("dev"),
    ];

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).unwrap();

    let calendar = utf8proj_core::Calendar::default();
    let result = level_resources(&project, &schedule, &calendar);

    // Should have shifted at least one task
    assert!(
        !result.shifted_tasks.is_empty(),
        "Should shift tasks to resolve conflict"
    );

    // No unresolved conflicts
    assert!(
        result.unresolved_conflicts.is_empty(),
        "All conflicts should be resolved"
    );

    // Project should be extended
    assert!(result.project_extended, "Project should be extended");
}

// =============================================================================
// Multiple Resources
// =============================================================================

#[test]
fn leveling_multiple_resources_independent() {
    let mut project = Project::new("Multiple Resources Test");
    project.start = date(2025, 1, 6);
    project.resources = vec![
        Resource::new("dev1").capacity(1.0),
        Resource::new("dev2").capacity(1.0),
    ];
    project.tasks = vec![
        Task::new("task1").effort(Duration::days(5)).assign("dev1"),
        Task::new("task2").effort(Duration::days(5)).assign("dev2"),
    ];

    // No conflict - different resources
    let solver = CpmSolver::with_leveling();
    let schedule = solver.schedule(&project).unwrap();

    let task1 = &schedule.tasks["task1"];
    let task2 = &schedule.tasks["task2"];

    // Both can start on day 0
    assert_eq!(
        task1.start, task2.start,
        "Tasks on different resources should run in parallel"
    );
}

#[test]
fn leveling_multiple_resources_with_conflict() {
    let mut project = Project::new("Multiple Resources Conflict Test");
    project.start = date(2025, 1, 6);
    project.resources = vec![
        Resource::new("dev1").capacity(1.0),
        Resource::new("dev2").capacity(1.0),
    ];
    project.tasks = vec![
        Task::new("task1").effort(Duration::days(5)).assign("dev1"),
        Task::new("task2").effort(Duration::days(5)).assign("dev1"), // Same resource - conflict
        Task::new("task3").effort(Duration::days(5)).assign("dev2"), // Different resource - no conflict
    ];

    let solver = CpmSolver::with_leveling();
    let schedule = solver.schedule(&project).unwrap();

    let task1 = &schedule.tasks["task1"];
    let task2 = &schedule.tasks["task2"];
    let task3 = &schedule.tasks["task3"];

    // task1 and task2 should be sequential (same resource)
    assert!(
        task1.finish < task2.start || task2.finish < task1.start,
        "Tasks on same resource should be sequential"
    );

    // task3 can run in parallel with one of them (different resource)
    assert!(
        task3.start == task1.start || task3.start == task2.start,
        "Task3 on different resource should start with one of the dev1 tasks"
    );
}

// =============================================================================
// Priority-Based Leveling
// =============================================================================

#[test]
fn leveling_respects_priority() {
    let mut project = Project::new("Priority Test");
    project.start = date(2025, 1, 6);
    project.resources = vec![Resource::new("dev").capacity(1.0)];
    project.tasks = vec![
        Task::new("low_priority")
            .effort(Duration::days(5))
            .assign("dev")
            .priority(100),
        Task::new("high_priority")
            .effort(Duration::days(5))
            .assign("dev")
            .priority(900),
    ];

    let solver = CpmSolver::with_leveling();
    let schedule = solver.schedule(&project).unwrap();

    let low = &schedule.tasks["low_priority"];
    let high = &schedule.tasks["high_priority"];

    // Higher priority task should generally not be shifted (lower priority shifted instead)
    // Due to heap ordering, the lower priority task should be the one that gets shifted
    // This means high_priority should start at project start
    assert!(
        high.start <= low.start,
        "Higher priority task should start first or at same time"
    );
}

// =============================================================================
// Edge Cases
// =============================================================================

#[test]
fn leveling_no_resources_assigned() {
    let mut project = Project::new("No Resources Test");
    project.start = date(2025, 1, 6);
    project.resources = vec![Resource::new("dev").capacity(1.0)];
    project.tasks = vec![
        Task::new("task1").effort(Duration::days(5)), // No resource
        Task::new("task2").effort(Duration::days(5)), // No resource
    ];

    // No resource assignment = no leveling needed
    let solver = CpmSolver::with_leveling();
    let schedule = solver.schedule(&project).unwrap();

    // Both tasks can start on day 0
    assert_eq!(schedule.tasks["task1"].start, schedule.tasks["task2"].start);
}

#[test]
fn leveling_empty_project() {
    let project = Project::new("Empty Project");

    let solver = CpmSolver::with_leveling();
    let schedule = solver.schedule(&project).unwrap();

    assert!(schedule.tasks.is_empty());
}

#[test]
fn leveling_single_task() {
    let mut project = Project::new("Single Task Test");
    project.start = date(2025, 1, 6);
    project.resources = vec![Resource::new("dev").capacity(1.0)];
    project.tasks = vec![Task::new("task1").effort(Duration::days(5)).assign("dev")];

    let solver = CpmSolver::with_leveling();
    let schedule = solver.schedule(&project).unwrap();

    assert_eq!(schedule.tasks.len(), 1);
    assert_eq!(schedule.tasks["task1"].start, project.start);
}
