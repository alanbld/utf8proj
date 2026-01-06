//! Integration tests for variance reporting functionality
//!
//! These tests verify that baseline vs forecast variance is computed correctly.

use chrono::NaiveDate;
use utf8proj_core::{Duration, Money, Project, Scheduler, Task};
use utf8proj_solver::CpmSolver;

/// Test that tasks with no progress have zero variance
#[test]
fn no_progress_zero_variance() {
    let mut project = Project::new("No Progress");
    project.start = NaiveDate::from_ymd_opt(2026, 1, 6).unwrap();

    // 10-day task with no progress
    let task = Task::new("work")
        .name("Work")
        .duration(Duration::days(10));
    project.tasks.push(task);

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).expect("Scheduling should succeed");

    let work = schedule.tasks.get("work").expect("work task should exist");

    // No progress = no variance
    assert_eq!(work.start_variance_days, 0);
    assert_eq!(work.finish_variance_days, 0);
    assert_eq!(work.baseline_start, work.forecast_start);
    assert_eq!(work.baseline_finish, work.forecast_finish);
}

/// Test that tasks with actual_start show start variance
#[test]
fn actual_start_shows_start_variance() {
    let mut project = Project::new("Late Start");
    project.start = NaiveDate::from_ymd_opt(2026, 1, 6).unwrap();

    // Task planned for Jan 6, but actually started Jan 8 (2 days late)
    let mut task = Task::new("work")
        .name("Work")
        .duration(Duration::days(5));
    task.actual_start = Some(NaiveDate::from_ymd_opt(2026, 1, 8).unwrap());
    task.complete = Some(50.0);
    project.tasks.push(task);

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).expect("Scheduling should succeed");

    let work = schedule.tasks.get("work").expect("work task should exist");

    // Started 2 days late
    assert_eq!(work.start_variance_days, 2);
    // Baseline: Jan 6
    assert_eq!(work.baseline_start, NaiveDate::from_ymd_opt(2026, 1, 6).unwrap());
    // Forecast: Jan 8 (actual start)
    assert_eq!(work.forecast_start, NaiveDate::from_ymd_opt(2026, 1, 8).unwrap());
}

/// Test that partial progress affects finish variance
#[test]
fn partial_progress_shows_finish_variance() {
    let mut project = Project::new("Slipping Task");
    project.start = NaiveDate::from_ymd_opt(2026, 1, 6).unwrap();

    // 10-day task at 10% complete, started 5 days late
    // Baseline: Jan 6 + 10 working days → ~Jan 19
    // Forecast: Started Jan 13 + 9 remaining days → much later
    // This creates actual slippage because the late start + little progress
    // means they'll finish well after the baseline
    let mut task = Task::new("work")
        .name("Work")
        .duration(Duration::days(10));
    task.actual_start = Some(NaiveDate::from_ymd_opt(2026, 1, 13).unwrap()); // 5 working days late
    task.complete = Some(10.0); // Only 10% done = 9 days remaining
    project.tasks.push(task);

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).expect("Scheduling should succeed");

    let work = schedule.tasks.get("work").expect("work task should exist");

    // Should have positive finish variance (running late)
    // Started very late with only 10% done = definitely slipping
    assert!(work.finish_variance_days > 0, "Task should be slipping: variance = {}", work.finish_variance_days);
    assert_eq!(work.percent_complete, 10);
    assert_eq!(work.remaining_duration.as_days() as i64, 9); // 90% of 10 days
}

/// Test that completed tasks show actual finish
#[test]
fn completed_task_uses_actual_finish() {
    let mut project = Project::new("Completed");
    project.start = NaiveDate::from_ymd_opt(2026, 1, 6).unwrap();

    // Task completed 2 days early
    let mut task = Task::new("work")
        .name("Work")
        .duration(Duration::days(10));
    task.complete = Some(100.0);
    task.actual_start = Some(NaiveDate::from_ymd_opt(2026, 1, 6).unwrap());
    task.actual_finish = Some(NaiveDate::from_ymd_opt(2026, 1, 15).unwrap()); // 2 days early
    project.tasks.push(task);

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).expect("Scheduling should succeed");

    let work = schedule.tasks.get("work").expect("work task should exist");

    // Completed 2 days early (negative variance)
    assert!(work.finish_variance_days < 0, "Task should be ahead of schedule");
    assert_eq!(work.forecast_finish, NaiveDate::from_ymd_opt(2026, 1, 15).unwrap());
}

/// Test variance with resource parallelism (effort vs duration)
#[test]
fn resource_parallelism_correct_variance() {
    let mut project = Project::new("Parallel");
    project.start = NaiveDate::from_ymd_opt(2026, 1, 6).unwrap();

    // Define two resources
    project.resources.push(utf8proj_core::Resource::new("dev1").name("Dev 1").rate(Money::new(100, "USD")));
    project.resources.push(utf8proj_core::Resource::new("dev2").name("Dev 2").rate(Money::new(100, "USD")));

    // 10d effort with 2 resources = 5d duration
    // No progress = no variance
    let task = Task::new("work")
        .name("Work")
        .effort(Duration::days(10))
        .assign("dev1")
        .assign("dev2");
    project.tasks.push(task);

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).expect("Scheduling should succeed");

    let work = schedule.tasks.get("work").expect("work task should exist");

    // No variance for task without progress
    assert_eq!(work.start_variance_days, 0);
    assert_eq!(work.finish_variance_days, 0);
    // Remaining should be 5d (scheduled duration), not 10d (effort)
    assert_eq!(work.remaining_duration.as_days() as i64, 5);
}

/// Test that early start shows negative start variance
#[test]
fn early_start_negative_variance() {
    let mut project = Project::new("Early Start");
    project.start = NaiveDate::from_ymd_opt(2026, 1, 6).unwrap();

    // Task started 2 days early (Jan 3 instead of Jan 6)
    let mut task = Task::new("work")
        .name("Work")
        .duration(Duration::days(5));
    task.actual_start = Some(NaiveDate::from_ymd_opt(2026, 1, 3).unwrap());
    task.complete = Some(20.0);
    project.tasks.push(task);

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).expect("Scheduling should succeed");

    let work = schedule.tasks.get("work").expect("work task should exist");

    // Started 3 days early (Jan 3 vs Jan 6 = -3 days)
    assert_eq!(work.start_variance_days, -3);
}
