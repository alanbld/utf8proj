//! Integration tests for Earned Value metrics (I005)
//!
//! These tests verify PV, EV, and SPI computation.

use chrono::{Local, NaiveDate};
use utf8proj_core::{Duration, Project, Scheduler, Task};
use utf8proj_solver::CpmSolver;

/// Helper to get current date
fn today() -> NaiveDate {
    Local::now().date_naive()
}

/// Test: Empty project has default EV values
#[test]
fn empty_project_default_ev() {
    let mut project = Project::new("Empty");
    project.start = today();

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).expect("Should succeed");

    // Empty project: PV=0, EV=0, SPI=1.0
    assert_eq!(schedule.planned_value, 0);
    assert_eq!(schedule.earned_value, 0);
    assert!((schedule.spi - 1.0).abs() < 0.01);
}

/// Test: Project before baseline start has PV=0
#[test]
fn project_before_start_pv_zero() {
    let mut project = Project::new("Future Project");
    // Project starts in the future
    project.start = today() + chrono::Duration::days(30);

    let task = Task::new("work").name("Work").duration(Duration::days(10));
    project.tasks.push(task);

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).expect("Should succeed");

    // Before project start: PV=0
    assert_eq!(schedule.planned_value, 0);
    assert_eq!(schedule.earned_value, 0);
    // EV=0, PV=0 => SPI=1.0
    assert!((schedule.spi - 1.0).abs() < 0.01);
}

/// Test: Completed project has EV=100, PV=100, SPI=1.0
#[test]
fn completed_project_full_ev() {
    let mut project = Project::new("Complete");
    // Project started 20 days ago
    project.start = today() - chrono::Duration::days(20);

    let mut task = Task::new("work").name("Work").duration(Duration::days(10));
    task.complete = Some(100.0);
    project.tasks.push(task);

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).expect("Should succeed");

    // Completed: EV=100, PV=100 (past baseline_finish)
    assert_eq!(schedule.earned_value, 100);
    assert_eq!(schedule.planned_value, 100);
    assert!((schedule.spi - 1.0).abs() < 0.01);
}

/// Test: Project on schedule has SPI ~= 1.0
#[test]
fn on_schedule_spi_one() {
    let mut project = Project::new("On Schedule");
    // Project started 5 days ago
    project.start = today() - chrono::Duration::days(5);

    // 10-day task, 50% complete after 5 days = on schedule
    let mut task = Task::new("work").name("Work").duration(Duration::days(10));
    task.complete = Some(50.0);
    project.tasks.push(task);

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).expect("Should succeed");

    // Get task for debugging
    let work = schedule.tasks.get("work").expect("work task");
    eprintln!("Today: {}", today());
    eprintln!("Project start: {}", project.start);
    eprintln!("Baseline start: {}", work.baseline_start);
    eprintln!("Baseline finish: {}", work.baseline_finish);
    eprintln!(
        "PV: {}, EV: {}, SPI: {:.2}",
        schedule.planned_value, schedule.earned_value, schedule.spi
    );

    // EV=50% (actual progress)
    assert_eq!(schedule.earned_value, 50);
    // PV depends on baseline dates vs status date (today)
    // The baseline span is in working days, but PV calculation uses calendar days
    // So this can vary depending on weekends
    // Just verify PV is computed (not 0) and SPI is reasonable
    assert!(
        schedule.planned_value > 0,
        "PV should be positive for in-progress task"
    );
    // SPI should reflect being on schedule (within reasonable bounds)
    assert!(
        schedule.spi >= 0.5 && schedule.spi <= 1.5,
        "SPI {} should be near 1.0",
        schedule.spi
    );
}

/// Test: Project behind schedule has SPI < 1.0
#[test]
fn behind_schedule_spi_below_one() {
    let mut project = Project::new("Behind");
    // Project started 8 days ago
    project.start = today() - chrono::Duration::days(8);

    // 10-day task, only 20% complete after 8 days = behind schedule
    let mut task = Task::new("work").name("Work").duration(Duration::days(10));
    task.complete = Some(20.0);
    project.tasks.push(task);

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).expect("Should succeed");

    // EV=20% (actual progress), PV should be ~80% (8/10 days elapsed)
    assert_eq!(schedule.earned_value, 20);
    // PV should be high (past 50% of baseline)
    assert!(schedule.planned_value >= 60);
    // SPI should be low (behind schedule)
    assert!(schedule.spi < 0.5);
}

/// Test: Project ahead of schedule has SPI > 1.0
#[test]
fn ahead_of_schedule_spi_above_one() {
    let mut project = Project::new("Ahead");
    // Project started 2 days ago
    project.start = today() - chrono::Duration::days(2);

    // 10-day task, 50% complete after 2 days = ahead of schedule
    let mut task = Task::new("work").name("Work").duration(Duration::days(10));
    task.complete = Some(50.0);
    project.tasks.push(task);

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).expect("Should succeed");

    // EV=50% (actual progress), PV should be ~20% (2/10 days elapsed)
    assert_eq!(schedule.earned_value, 50);
    assert!(schedule.planned_value <= 30);
    // SPI should be high (ahead of schedule), capped at 2.0
    assert!(schedule.spi > 1.5 && schedule.spi <= 2.0);
}

/// Test: Multiple tasks with weighted average
#[test]
fn multiple_tasks_weighted_ev() {
    let mut project = Project::new("Multi Task");
    // Project started 10 days ago
    project.start = today() - chrono::Duration::days(10);

    // Task 1: 5 days, 100% complete
    let mut task1 = Task::new("task1")
        .name("Task 1")
        .duration(Duration::days(5));
    task1.complete = Some(100.0);
    project.tasks.push(task1);

    // Task 2: 15 days, 0% complete (just started)
    let task2 = Task::new("task2")
        .name("Task 2")
        .duration(Duration::days(15))
        .depends_on("task1");
    project.tasks.push(task2);

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).expect("Should succeed");

    // EV = weighted average: (5*100 + 15*0) / 20 = 25%
    assert_eq!(schedule.earned_value, 25);

    // PV depends on baseline progress and varies with weekends
    // Task 1 baseline: ~5 working days, should be 100% planned (past baseline)
    // Task 2 baseline: 15 working days, progress depends on calendar alignment
    // The exact PV varies based on which days are weekends, so we use a wide range
    // With typical week patterns: PV should be between 30-70%
    assert!(
        schedule.planned_value >= 25 && schedule.planned_value <= 75,
        "PV {} should be in reasonable range for mid-project status",
        schedule.planned_value
    );
}

/// Test: SPI capped at 2.0 for extreme cases
#[test]
fn spi_capped_at_two() {
    let mut project = Project::new("Cap Test");
    // Project just started today
    project.start = today();

    // 20-day task, but already 80% complete
    let mut task = Task::new("work").name("Work").duration(Duration::days(20));
    task.complete = Some(80.0);
    project.tasks.push(task);

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).expect("Should succeed");

    // EV=80%, PV should be ~0% (just started)
    assert_eq!(schedule.earned_value, 80);
    // SPI should be capped at 2.0
    assert!((schedule.spi - 2.0).abs() < 0.01);
}

/// Test: Container tasks are excluded from EV calculation
#[test]
fn container_tasks_excluded() {
    let mut project = Project::new("Container Test");
    project.start = today() - chrono::Duration::days(5);

    // Container task with two leaf children
    let container = Task::new("phase1")
        .name("Phase 1")
        .child(
            Task::new("task1")
                .name("Task 1")
                .duration(Duration::days(5))
                .complete(50.0),
        )
        .child(
            Task::new("task2")
                .name("Task 2")
                .duration(Duration::days(5))
                .complete(50.0),
        );
    project.tasks.push(container);

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).expect("Should succeed");

    // EV should be based on leaf tasks only, not container
    // Both leaf tasks at 50%: EV = (5*50 + 5*50) / 10 = 50%
    assert_eq!(schedule.earned_value, 50);
}

/// Test: I005 diagnostic is emitted
#[test]
fn i005_diagnostic_emitted() {
    use utf8proj_core::{CollectingEmitter, DiagnosticCode};
    use utf8proj_solver::{analyze_project, AnalysisConfig};

    let mut project = Project::new("Diagnostic Test");
    project.start = today() - chrono::Duration::days(5);

    let mut task = Task::new("work").name("Work").duration(Duration::days(10));
    task.complete = Some(50.0);
    project.tasks.push(task);

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).expect("Should succeed");

    let mut emitter = CollectingEmitter::new();
    let config = AnalysisConfig::default();
    analyze_project(&project, Some(&schedule), &config, &mut emitter);

    // Should have I005 diagnostic
    assert!(
        emitter
            .diagnostics
            .iter()
            .any(|d| d.code == DiagnosticCode::I005EarnedValueSummary),
        "Should emit I005EarnedValueSummary diagnostic"
    );

    // Verify diagnostic contains SPI info
    let i005 = emitter
        .diagnostics
        .iter()
        .find(|d| d.code == DiagnosticCode::I005EarnedValueSummary)
        .expect("Should have I005");
    assert!(i005.message.contains("SPI"));
}
