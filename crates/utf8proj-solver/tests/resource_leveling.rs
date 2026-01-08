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
// RFC-0003 Required Tests
// =============================================================================

/// RFC-0003: Determinism test - same input must produce identical output
#[test]
fn leveling_is_deterministic() {
    let mut project = Project::new("Determinism Test");
    project.start = date(2025, 1, 6); // Monday
    project.resources = vec![Resource::new("dev").capacity(1.0)];

    // Create multiple tasks that will conflict (all assigned to same resource)
    project.tasks = vec![
        Task::new("task_a").effort(Duration::days(3)).assign("dev"),
        Task::new("task_b").effort(Duration::days(3)).assign("dev"),
        Task::new("task_c").effort(Duration::days(3)).assign("dev"),
        Task::new("task_d").effort(Duration::days(3)).assign("dev"),
    ];

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).unwrap();
    let calendar = utf8proj_core::Calendar::default();

    // Run leveling multiple times and verify determinism
    let result1 = level_resources(&project, &schedule, &calendar);
    let result2 = level_resources(&project, &schedule, &calendar);
    let result3 = level_resources(&project, &schedule, &calendar);

    // All results must be identical
    assert_eq!(result1.shifted_tasks.len(), result2.shifted_tasks.len());
    assert_eq!(result2.shifted_tasks.len(), result3.shifted_tasks.len());

    for (i, (s1, s2)) in result1.shifted_tasks.iter().zip(&result2.shifted_tasks).enumerate() {
        assert_eq!(
            s1.task_id, s2.task_id,
            "Run 1 vs 2: Task order differs at index {}: {} vs {}",
            i, s1.task_id, s2.task_id
        );
        assert_eq!(
            s1.days_shifted, s2.days_shifted,
            "Run 1 vs 2: Days shifted differs for task {}",
            s1.task_id
        );
        assert_eq!(
            s1.new_start, s2.new_start,
            "Run 1 vs 2: New start differs for task {}",
            s1.task_id
        );
    }

    for (i, (s2, s3)) in result2.shifted_tasks.iter().zip(&result3.shifted_tasks).enumerate() {
        assert_eq!(
            s2.task_id, s3.task_id,
            "Run 2 vs 3: Task order differs at index {}: {} vs {}",
            i, s2.task_id, s3.task_id
        );
    }

    // Project end must be identical
    assert_eq!(
        result1.new_project_end, result2.new_project_end,
        "Project end differs between runs"
    );
    assert_eq!(
        result2.new_project_end, result3.new_project_end,
        "Project end differs between runs"
    );
}

/// RFC-0003: No-op test - no overallocation means leveled == original
#[test]
fn leveling_noop_when_no_conflict() {
    let mut project = Project::new("No-Op Test");
    project.start = date(2025, 1, 6);
    project.resources = vec![
        Resource::new("dev1").capacity(1.0),
        Resource::new("dev2").capacity(1.0),
    ];

    // Tasks on different resources - no conflict
    project.tasks = vec![
        Task::new("task1").effort(Duration::days(5)).assign("dev1"),
        Task::new("task2").effort(Duration::days(5)).assign("dev2"),
    ];

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).unwrap();
    let calendar = utf8proj_core::Calendar::default();

    let result = level_resources(&project, &schedule, &calendar);

    // No tasks should be shifted
    assert!(
        result.shifted_tasks.is_empty(),
        "No tasks should be shifted when there's no conflict"
    );

    // Project end should be unchanged
    assert_eq!(
        result.original_schedule.project_end, result.leveled_schedule.project_end,
        "Project end should be unchanged"
    );

    // Original and leveled schedules should have same task dates
    for (task_id, orig_task) in &result.original_schedule.tasks {
        let leveled_task = &result.leveled_schedule.tasks[task_id];
        assert_eq!(
            orig_task.start, leveled_task.start,
            "Task {} start should be unchanged",
            task_id
        );
        assert_eq!(
            orig_task.finish, leveled_task.finish,
            "Task {} finish should be unchanged",
            task_id
        );
    }
}

/// RFC-0003: Critical path preservation - non-critical delayed before critical
#[test]
fn leveling_preserves_critical_path_priority() {
    let mut project = Project::new("Critical Path Test");
    project.start = date(2025, 1, 6);
    project.resources = vec![Resource::new("dev").capacity(1.0)];

    // Two tasks: one critical (on longest path), one with slack
    project.tasks = vec![
        // Critical task - depends on nothing, successor depends on it
        Task::new("critical").effort(Duration::days(5)).assign("dev"),
        // Non-critical task - parallel, has slack
        Task::new("non_critical")
            .effort(Duration::days(3))
            .assign("dev")
            .priority(100), // Lower priority
    ];

    let solver = CpmSolver::with_leveling();
    let schedule = solver.schedule(&project).unwrap();

    // Both tasks assigned to same resource - one must be shifted
    // The non-critical task should be shifted, not the critical one
    let critical = &schedule.tasks["critical"];
    let non_critical = &schedule.tasks["non_critical"];

    // After leveling, they should not overlap
    let no_overlap = critical.finish < non_critical.start || non_critical.finish < critical.start;
    assert!(no_overlap, "Tasks should not overlap after leveling");

    // Critical task should start at project start (not delayed)
    assert_eq!(
        critical.start, project.start,
        "Critical task should start at project start"
    );
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

// =============================================================================
// Diagnostic Emission Tests (L001-L004)
// =============================================================================

/// Verify L001 diagnostic is emitted when overallocation is resolved
#[test]
fn leveling_emits_l001_on_resolution() {
    use utf8proj_core::DiagnosticCode;

    let mut project = Project::new("L001 Test");
    project.start = date(2025, 1, 6);
    project.resources = vec![Resource::new("dev").capacity(1.0)];
    project.tasks = vec![
        Task::new("task1").effort(Duration::days(5)).assign("dev"),
        Task::new("task2").effort(Duration::days(5)).assign("dev"),
    ];

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).unwrap();
    let calendar = utf8proj_core::Calendar::default();

    let result = level_resources(&project, &schedule, &calendar);

    // Should have shifted at least one task
    assert!(!result.shifted_tasks.is_empty(), "Should have shifted tasks");

    // Should have emitted L001 diagnostic
    let l001_count = result
        .diagnostics
        .iter()
        .filter(|d| d.code == DiagnosticCode::L001OverallocationResolved)
        .count();

    assert!(
        l001_count > 0,
        "Should emit L001 diagnostic when resolving overallocation"
    );
}

/// Verify L003 diagnostic is emitted when project duration increases
#[test]
fn leveling_emits_l003_on_duration_increase() {
    use utf8proj_core::DiagnosticCode;

    let mut project = Project::new("L003 Test");
    project.start = date(2025, 1, 6);
    project.resources = vec![Resource::new("dev").capacity(1.0)];
    project.tasks = vec![
        Task::new("task1").effort(Duration::days(5)).assign("dev"),
        Task::new("task2").effort(Duration::days(5)).assign("dev"),
    ];

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).unwrap();
    let calendar = utf8proj_core::Calendar::default();

    let result = level_resources(&project, &schedule, &calendar);

    // Project should be extended
    assert!(result.project_extended, "Project should be extended");

    // Should have emitted L003 diagnostic
    let l003_exists = result
        .diagnostics
        .iter()
        .any(|d| d.code == DiagnosticCode::L003DurationIncreased);

    assert!(
        l003_exists,
        "Should emit L003 diagnostic when project duration increases"
    );
}

// =============================================================================
// LevelingReason and Metrics Verification
// =============================================================================

/// Verify LevelingReason::ResourceOverallocated is populated correctly
#[test]
fn leveling_reason_resource_overallocated() {
    use utf8proj_solver::LevelingReason;

    let mut project = Project::new("Reason Test");
    project.start = date(2025, 1, 6);
    project.resources = vec![Resource::new("alice").capacity(1.0)];
    project.tasks = vec![
        Task::new("task1").effort(Duration::days(3)).assign("alice"),
        Task::new("task2").effort(Duration::days(3)).assign("alice"),
    ];

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).unwrap();
    let calendar = utf8proj_core::Calendar::default();

    let result = level_resources(&project, &schedule, &calendar);

    // At least one task should have been shifted
    assert!(!result.shifted_tasks.is_empty());

    // Verify the reason is ResourceOverallocated with correct resource
    let shifted = &result.shifted_tasks[0];
    match &shifted.reason {
        LevelingReason::ResourceOverallocated {
            resource,
            peak_demand,
            capacity,
            dates,
        } => {
            assert_eq!(resource, "alice", "Resource should be 'alice'");
            assert!(*peak_demand > *capacity, "Peak demand should exceed capacity");
            assert!(!dates.is_empty(), "Should have conflict dates");
        }
        LevelingReason::DependencyChain { .. } => {
            panic!("Expected ResourceOverallocated, got DependencyChain");
        }
    }

    // Verify resources_involved is populated
    assert!(
        shifted.resources_involved.contains(&"alice".to_string()),
        "resources_involved should contain 'alice'"
    );
}

/// Verify LevelingMetrics are calculated correctly
#[test]
fn leveling_metrics_calculated() {
    let mut project = Project::new("Metrics Test");
    project.start = date(2025, 1, 6);
    project.resources = vec![Resource::new("dev").capacity(1.0)];
    project.tasks = vec![
        Task::new("task1").effort(Duration::days(5)).assign("dev"),
        Task::new("task2").effort(Duration::days(5)).assign("dev"),
        Task::new("task3").effort(Duration::days(5)).assign("dev"),
    ];

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).unwrap();
    let calendar = utf8proj_core::Calendar::default();

    let result = level_resources(&project, &schedule, &calendar);

    // Verify metrics
    assert!(
        result.metrics.tasks_delayed > 0,
        "Should have delayed tasks"
    );
    assert!(
        result.metrics.total_delay_days > 0,
        "Should have total delay days"
    );
    assert!(
        result.metrics.project_duration_increase > 0,
        "Should have duration increase"
    );

    // Peak utilization should have decreased (from >1.0 to <=1.0)
    assert!(
        result.metrics.peak_utilization_before > 1.0,
        "Peak utilization before should be >100% (overallocated)"
    );
    assert!(
        result.metrics.peak_utilization_after <= 1.0,
        "Peak utilization after should be <=100% (resolved)"
    );
}

/// Verify original schedule is preserved in result
#[test]
fn leveling_preserves_original_schedule() {
    let mut project = Project::new("Original Preserved Test");
    project.start = date(2025, 1, 6);
    project.resources = vec![Resource::new("dev").capacity(1.0)];
    project.tasks = vec![
        Task::new("task1").effort(Duration::days(5)).assign("dev"),
        Task::new("task2").effort(Duration::days(5)).assign("dev"),
    ];

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).unwrap();
    let original_task1_start = schedule.tasks["task1"].start;
    let original_task2_start = schedule.tasks["task2"].start;
    let original_end = schedule.project_end;

    let calendar = utf8proj_core::Calendar::default();
    let result = level_resources(&project, &schedule, &calendar);

    // Original schedule in result should match input
    assert_eq!(
        result.original_schedule.tasks["task1"].start,
        original_task1_start,
        "Original task1 start should be preserved"
    );
    assert_eq!(
        result.original_schedule.tasks["task2"].start,
        original_task2_start,
        "Original task2 start should be preserved"
    );
    assert_eq!(
        result.original_schedule.project_end, original_end,
        "Original project end should be preserved"
    );

    // Leveled schedule should be different (extended)
    assert!(
        result.leveled_schedule.project_end > result.original_schedule.project_end,
        "Leveled schedule should extend beyond original"
    );
}
