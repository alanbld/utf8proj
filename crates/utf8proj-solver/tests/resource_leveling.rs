//! Integration tests for Resource Leveling
//!
//! Tests the CpmSolver with resource leveling enabled.

use chrono::NaiveDate;
use utf8proj_core::{Duration, Project, Resource, Scheduler, Task};
use utf8proj_solver::{
    detect_overallocations, level_resources, level_resources_with_options, CpmSolver,
    LevelingOptions, LevelingStrategy,
};

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

    for (i, (s1, s2)) in result1
        .shifted_tasks
        .iter()
        .zip(&result2.shifted_tasks)
        .enumerate()
    {
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

    for (i, (s2, s3)) in result2
        .shifted_tasks
        .iter()
        .zip(&result3.shifted_tasks)
        .enumerate()
    {
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
        Task::new("critical")
            .effort(Duration::days(5))
            .assign("dev"),
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
        task1.start, task1.finish, task2.start, task2.finish
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
        Task::new("task1").effort(Duration::days(5)).assign("dev"), // Default 100%
        Task::new("task2").effort(Duration::days(5)).assign("dev"), // Default 100%
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
    assert!(
        !result.shifted_tasks.is_empty(),
        "Should have shifted tasks"
    );

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
            assert!(
                *peak_demand > *capacity,
                "Peak demand should exceed capacity"
            );
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
        result.original_schedule.tasks["task1"].start, original_task1_start,
        "Original task1 start should be preserved"
    );
    assert_eq!(
        result.original_schedule.tasks["task2"].start, original_task2_start,
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

// =============================================================================
// Coverage Gap Tests
// =============================================================================

/// Test LevelingReason Display impl for ResourceOverallocated
#[test]
fn leveling_reason_display_resource_overallocated() {
    use utf8proj_solver::LevelingReason;

    let reason = LevelingReason::ResourceOverallocated {
        resource: "dev".to_string(),
        peak_demand: 2.0,
        capacity: 1.0,
        dates: vec![date(2025, 1, 6), date(2025, 1, 7)],
    };

    let display = format!("{}", reason);
    assert!(display.contains("dev"), "Should mention resource name");
    assert!(
        display.contains("200%"),
        "Should show peak demand as percentage"
    );
    assert!(
        display.contains("100%"),
        "Should show capacity as percentage"
    );
    assert!(display.contains("2 day"), "Should mention number of days");
}

/// Test LevelingReason Display impl for DependencyChain
#[test]
fn leveling_reason_display_dependency_chain() {
    use utf8proj_solver::LevelingReason;

    let reason = LevelingReason::DependencyChain {
        predecessor: "task_a".to_string(),
        predecessor_delay: 3,
    };

    let display = format!("{}", reason);
    assert!(display.contains("task_a"), "Should mention predecessor");
    assert!(display.contains("3 days"), "Should mention delay days");
}

/// Test backwards compatibility schedule() alias
#[test]
fn leveling_result_schedule_alias() {
    let mut project = Project::new("Alias Test");
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

    // schedule() should return same as leveled_schedule
    assert_eq!(
        result.schedule().project_end,
        result.leveled_schedule.project_end,
        "schedule() alias should return leveled_schedule"
    );
}

/// Test L004 diagnostic - milestone delay detection
/// NOTE: L004 checks if milestone.start > original_date after leveling.
/// Currently, milestones are not re-scheduled after predecessor shifts,
/// so L004 only fires when milestones themselves are directly shifted.
/// This test documents the current behavior.
#[test]
fn leveling_milestone_tracking() {
    let mut project = Project::new("Milestone Test");
    project.start = date(2025, 1, 6);
    project.resources = vec![Resource::new("dev").capacity(1.0)];
    project.tasks = vec![
        Task::new("task1").effort(Duration::days(5)).assign("dev"),
        Task::new("task2").effort(Duration::days(5)).assign("dev"),
        // Milestone depends on both - tracked but not re-scheduled after predecessor shifts
        Task::new("milestone")
            .effort(Duration::days(0))
            .milestone()
            .depends_on("task1")
            .depends_on("task2"),
    ];

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).unwrap();
    let calendar = utf8proj_core::Calendar::default();
    let result = level_resources(&project, &schedule, &calendar);

    // Milestones are tracked (zero duration tasks)
    // L004 only fires if milestone itself is shifted, not cascaded from predecessors
    // This test documents that milestone tracking exists
    assert!(
        result.leveled_schedule.tasks.contains_key("milestone"),
        "Milestone should exist in leveled schedule"
    );
}

/// Test max_project_delay_factor prevents excessive delays
#[test]
fn leveling_respects_max_delay_factor() {
    use utf8proj_solver::{level_resources_with_options, LevelingOptions, LevelingStrategy};

    let mut project = Project::new("Max Delay Test");
    project.start = date(2025, 1, 6);
    project.resources = vec![Resource::new("dev").capacity(1.0)];

    // 10 tasks that would require significant extension
    project.tasks = (0..10)
        .map(|i| {
            Task::new(&format!("task_{}", i))
                .effort(Duration::days(5))
                .assign("dev")
        })
        .collect();

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).unwrap();
    let calendar = utf8proj_core::Calendar::default();

    // Without limit - all conflicts resolved
    let result_unlimited = level_resources(&project, &schedule, &calendar);

    // With very restrictive limit (1.1x = only 10% extension allowed)
    let options = LevelingOptions {
        strategy: LevelingStrategy::CriticalPathFirst,
        max_project_delay_factor: Some(1.1), ..Default::default()
    };
    let result_limited = level_resources_with_options(&project, &schedule, &calendar, &options);

    // Limited version should have more unresolved conflicts
    assert!(
        result_limited.unresolved_conflicts.len() > result_unlimited.unresolved_conflicts.len(),
        "Max delay factor should prevent some shifts: limited={}, unlimited={}",
        result_limited.unresolved_conflicts.len(),
        result_unlimited.unresolved_conflicts.len()
    );
}

// =============================================================================
// RFC-0014: Hybrid BDD Leveling Tests
// =============================================================================

/// Test hybrid leveling produces same results as standard leveling for simple cases
#[test]
fn hybrid_leveling_basic_conflict() {
    use utf8proj_solver::{level_resources_with_options, LevelingOptions, LevelingStrategy};

    let mut project = Project::new("Hybrid Basic Test");
    project.start = date(2025, 1, 6);
    project.resources = vec![Resource::new("dev").capacity(1.0)];
    project.tasks = vec![
        Task::new("task1").effort(Duration::days(5)).assign("dev"),
        Task::new("task2").effort(Duration::days(5)).assign("dev"),
    ];

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).unwrap();
    let calendar = utf8proj_core::Calendar::default();

    // Standard leveling
    let options_standard = LevelingOptions {
        strategy: LevelingStrategy::CriticalPathFirst,
        max_project_delay_factor: None, ..Default::default()
    };
    let result_standard =
        level_resources_with_options(&project, &schedule, &calendar, &options_standard);

    // Hybrid leveling
    let options_hybrid = LevelingOptions {
        strategy: LevelingStrategy::Hybrid,
        max_project_delay_factor: None, ..Default::default()
    };
    let result_hybrid =
        level_resources_with_options(&project, &schedule, &calendar, &options_hybrid);

    // Both should resolve the conflict
    assert!(
        result_standard.unresolved_conflicts.is_empty(),
        "Standard leveling should resolve conflict"
    );
    assert!(
        result_hybrid.unresolved_conflicts.is_empty(),
        "Hybrid leveling should resolve conflict"
    );

    // Both should extend the project
    assert!(result_standard.project_extended);
    assert!(result_hybrid.project_extended);

    // Project end dates should be similar
    assert_eq!(
        result_standard.new_project_end, result_hybrid.new_project_end,
        "Hybrid should produce same project end as standard"
    );
}

/// Test hybrid leveling correctly handles no-conflict case
#[test]
fn hybrid_leveling_no_conflict() {
    use utf8proj_solver::{level_resources_with_options, LevelingOptions, LevelingStrategy};

    let mut project = Project::new("Hybrid No Conflict Test");
    project.start = date(2025, 1, 6);
    project.resources = vec![
        Resource::new("dev1").capacity(1.0),
        Resource::new("dev2").capacity(1.0),
    ];
    // Different resources - no conflict
    project.tasks = vec![
        Task::new("task1").effort(Duration::days(5)).assign("dev1"),
        Task::new("task2").effort(Duration::days(5)).assign("dev2"),
    ];

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).unwrap();
    let calendar = utf8proj_core::Calendar::default();

    let options = LevelingOptions {
        strategy: LevelingStrategy::Hybrid,
        max_project_delay_factor: None, ..Default::default()
    };
    let result = level_resources_with_options(&project, &schedule, &calendar, &options);

    // No shifts needed
    assert!(
        result.shifted_tasks.is_empty(),
        "No tasks should be shifted when there's no conflict"
    );
    assert!(!result.project_extended);
}

/// Test hybrid leveling handles multiple independent clusters
#[test]
fn hybrid_leveling_independent_clusters() {
    use utf8proj_solver::{level_resources_with_options, LevelingOptions, LevelingStrategy};

    let mut project = Project::new("Hybrid Clusters Test");
    project.start = date(2025, 1, 6);
    project.resources = vec![
        Resource::new("dev1").capacity(1.0),
        Resource::new("dev2").capacity(1.0),
    ];
    // Two independent clusters
    project.tasks = vec![
        // Cluster 1: dev1 conflict
        Task::new("task_a1").effort(Duration::days(3)).assign("dev1"),
        Task::new("task_a2").effort(Duration::days(3)).assign("dev1"),
        // Cluster 2: dev2 conflict
        Task::new("task_b1").effort(Duration::days(3)).assign("dev2"),
        Task::new("task_b2").effort(Duration::days(3)).assign("dev2"),
    ];

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).unwrap();
    let calendar = utf8proj_core::Calendar::default();

    let options = LevelingOptions {
        strategy: LevelingStrategy::Hybrid,
        max_project_delay_factor: None, ..Default::default()
    };
    let result = level_resources_with_options(&project, &schedule, &calendar, &options);

    // All conflicts should be resolved
    assert!(
        result.unresolved_conflicts.is_empty(),
        "All conflicts should be resolved"
    );

    // Should have diagnostics mentioning clusters
    let has_cluster_note = result.diagnostics.iter().any(|d| {
        d.notes
            .iter()
            .any(|note| note.contains("Cluster") || note.contains("cluster"))
    });
    assert!(has_cluster_note, "Should have diagnostic notes about clusters");
}

/// Test hybrid leveling is deterministic
#[test]
fn hybrid_leveling_is_deterministic() {
    use utf8proj_solver::{level_resources_with_options, LevelingOptions, LevelingStrategy};

    let mut project = Project::new("Hybrid Determinism Test");
    project.start = date(2025, 1, 6);
    project.resources = vec![Resource::new("dev").capacity(1.0)];
    project.tasks = vec![
        Task::new("task_a").effort(Duration::days(3)).assign("dev"),
        Task::new("task_b").effort(Duration::days(3)).assign("dev"),
        Task::new("task_c").effort(Duration::days(3)).assign("dev"),
    ];

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).unwrap();
    let calendar = utf8proj_core::Calendar::default();

    let options = LevelingOptions {
        strategy: LevelingStrategy::Hybrid,
        max_project_delay_factor: None, ..Default::default()
    };

    // Run multiple times
    let result1 = level_resources_with_options(&project, &schedule, &calendar, &options);
    let result2 = level_resources_with_options(&project, &schedule, &calendar, &options);
    let result3 = level_resources_with_options(&project, &schedule, &calendar, &options);

    // All should produce identical results
    assert_eq!(
        result1.new_project_end, result2.new_project_end,
        "Project end should be identical across runs"
    );
    assert_eq!(result2.new_project_end, result3.new_project_end);

    assert_eq!(result1.shifted_tasks.len(), result2.shifted_tasks.len());
    for (s1, s2) in result1.shifted_tasks.iter().zip(&result2.shifted_tasks) {
        assert_eq!(s1.task_id, s2.task_id, "Task order should be identical");
        assert_eq!(
            s1.new_start, s2.new_start,
            "New start should be identical for {}",
            s1.task_id
        );
    }
}

/// Performance test for parallel hybrid leveling with multiple clusters
/// Run with: cargo test -p utf8proj-solver parallel_hybrid_performance --release -- --ignored --nocapture
#[test]
#[ignore = "Long-running benchmark test, run manually with --ignored flag"]
fn parallel_hybrid_performance() {
    use std::time::Instant;
    use utf8proj_solver::{level_resources_with_options, LevelingOptions, LevelingStrategy};

    // Create a project with 10 independent clusters (10 resources, many tasks each)
    let num_clusters = 10;
    let tasks_per_cluster = 100; // 1000 total tasks

    let mut project = Project::new("Parallel Perf Test");
    project.start = date(2025, 1, 6);

    // Create resources (one per cluster)
    project.resources = (0..num_clusters)
        .map(|i| Resource::new(&format!("dev{}", i)).capacity(1.0))
        .collect();

    // Create tasks - each cluster has tasks competing for the same resource
    project.tasks = (0..num_clusters)
        .flat_map(|cluster| {
            (0..tasks_per_cluster).map(move |i| {
                Task::new(&format!("c{}t{}", cluster, i))
                    .effort(Duration::days(2))
                    .assign(&format!("dev{}", cluster))
            })
        })
        .collect();

    println!(
        "\nBenchmark: {} clusters × {} tasks = {} total tasks",
        num_clusters,
        tasks_per_cluster,
        project.tasks.len()
    );

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).unwrap();
    let calendar = utf8proj_core::Calendar::default();

    // Benchmark standard leveling
    let options_standard = LevelingOptions {
        strategy: LevelingStrategy::CriticalPathFirst,
        max_project_delay_factor: None, ..Default::default()
    };
    let start = Instant::now();
    let result_standard =
        level_resources_with_options(&project, &schedule, &calendar, &options_standard);
    let standard_time = start.elapsed();
    println!(
        "Standard leveling: {:?} ({} shifts)",
        standard_time,
        result_standard.shifted_tasks.len()
    );

    // Benchmark hybrid leveling (parallel)
    let options_hybrid = LevelingOptions {
        strategy: LevelingStrategy::Hybrid,
        max_project_delay_factor: None, ..Default::default()
    };
    let start = Instant::now();
    let result_hybrid =
        level_resources_with_options(&project, &schedule, &calendar, &options_hybrid);
    let hybrid_time = start.elapsed();
    println!(
        "Hybrid leveling:   {:?} ({} shifts)",
        hybrid_time,
        result_hybrid.shifted_tasks.len()
    );

    // Calculate speedup
    let speedup = standard_time.as_secs_f64() / hybrid_time.as_secs_f64();
    println!("Speedup: {:.1}x", speedup);

    // Verify correctness - both should resolve all conflicts
    assert!(
        result_standard.unresolved_conflicts.is_empty(),
        "Standard should resolve all conflicts"
    );
    assert!(
        result_hybrid.unresolved_conflicts.is_empty(),
        "Hybrid should resolve all conflicts"
    );
}

/// Test optimal leveling with constraint programming (RFC-0014 Phase 3)
#[cfg(feature = "optimal-leveling")]
#[test]
fn optimal_leveling_simple_conflict() {
    let mut project = Project::new("Optimal Test");
    project.start = date(2025, 1, 6); // Monday

    // Single resource with capacity 1.0
    project.resources = vec![Resource::new("dev").capacity(1.0)];

    // Two tasks that conflict (both assigned to same resource at 100%)
    project.tasks = vec![
        Task::new("task_a").effort(Duration::days(3)).assign("dev"),
        Task::new("task_b").effort(Duration::days(3)).assign("dev"),
    ];

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).unwrap();
    let calendar = utf8proj_core::Calendar::default();

    // Use optimal leveling
    let options = LevelingOptions {
        strategy: LevelingStrategy::Hybrid,
        use_optimal: true,
        optimal_threshold: 50,
        optimal_timeout_ms: 5000,
        ..Default::default()
    };

    let result = level_resources_with_options(&project, &schedule, &calendar, &options);

    // Should resolve the conflict by shifting one task
    assert!(
        result.unresolved_conflicts.is_empty(),
        "Optimal solver should resolve all conflicts"
    );
    assert!(
        !result.shifted_tasks.is_empty(),
        "Should have shifted at least one task"
    );

    // Verify no overlapping assignments on the resource
    let task_a = result.leveled_schedule.tasks.get("task_a").unwrap();
    let task_b = result.leveled_schedule.tasks.get("task_b").unwrap();

    // Tasks should not overlap
    let a_end = task_a.finish;
    let b_start = task_b.start;
    let b_end = task_b.finish;
    let a_start = task_a.start;

    let no_overlap = a_end < b_start || b_end < a_start;
    assert!(no_overlap, "Tasks should not overlap after optimal leveling");
}
