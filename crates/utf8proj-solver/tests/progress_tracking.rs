//! Integration tests for progress tracking functionality

use chrono::NaiveDate;
use utf8proj_core::{Duration, Project, Scheduler, Task, TaskStatus};
use utf8proj_solver::CpmSolver;

/// Test that progress tracking fields are calculated correctly
#[test]
fn schedule_includes_progress_tracking() {
    let mut project = Project::new("Progress Test");
    project.start = NaiveDate::from_ymd_opt(2026, 1, 6).unwrap();

    // 10-day task at 60% complete
    let mut task1 = Task::new("design")
        .name("Design")
        .duration(Duration::days(10));
    task1.complete = Some(60.0);
    task1.actual_start = Some(NaiveDate::from_ymd_opt(2026, 1, 6).unwrap());
    project.tasks.push(task1);

    // Dependent task - not started
    let task2 = Task::new("implement")
        .name("Implementation")
        .duration(Duration::days(20))
        .depends_on("design");
    project.tasks.push(task2);

    // Schedule the project
    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).expect("Scheduling should succeed");

    // Check design task progress
    let design = schedule.tasks.get("design").expect("design task should exist");
    assert_eq!(design.percent_complete, 60);
    assert_eq!(design.status, TaskStatus::InProgress);
    assert_eq!(design.remaining_duration.as_days() as i64, 4); // 40% of 10 days

    // Check implement task progress
    let implement = schedule.tasks.get("implement").expect("implement task should exist");
    assert_eq!(implement.percent_complete, 0);
    assert_eq!(implement.status, TaskStatus::NotStarted);
    assert_eq!(implement.remaining_duration.as_days() as i64, 20);
}

/// Test that completed tasks are reflected correctly
#[test]
fn completed_task_has_zero_remaining() {
    let mut project = Project::new("Completed Test");
    project.start = NaiveDate::from_ymd_opt(2026, 1, 6).unwrap();

    let mut task = Task::new("done")
        .name("Completed Task")
        .duration(Duration::days(5));
    task.complete = Some(100.0);
    task.actual_start = Some(NaiveDate::from_ymd_opt(2026, 1, 6).unwrap());
    task.actual_finish = Some(NaiveDate::from_ymd_opt(2026, 1, 10).unwrap());
    project.tasks.push(task);

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).expect("Scheduling should succeed");

    let done = schedule.tasks.get("done").expect("done task should exist");
    assert_eq!(done.percent_complete, 100);
    assert_eq!(done.status, TaskStatus::Complete);
    assert_eq!(done.remaining_duration.as_days() as i64, 0);
    assert_eq!(done.forecast_finish, NaiveDate::from_ymd_opt(2026, 1, 10).unwrap());
}

/// Test that actual_start overrides planned start for forecast
#[test]
fn actual_start_used_for_forecast() {
    let mut project = Project::new("Actual Start Test");
    project.start = NaiveDate::from_ymd_opt(2026, 1, 6).unwrap();

    let mut task = Task::new("started")
        .name("Started Late")
        .duration(Duration::days(10));
    // Started 5 days late
    task.actual_start = Some(NaiveDate::from_ymd_opt(2026, 1, 13).unwrap());
    task.complete = Some(50.0);
    project.tasks.push(task);

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).expect("Scheduling should succeed");

    let started = schedule.tasks.get("started").expect("started task should exist");
    // Forecast start should use actual_start
    assert_eq!(started.forecast_start, NaiveDate::from_ymd_opt(2026, 1, 13).unwrap());
    assert_eq!(started.remaining_duration.as_days() as i64, 5); // 50% of 10 days
}

/// Test explicit status override
#[test]
fn explicit_status_overrides_derived() {
    let mut project = Project::new("Status Override Test");
    project.start = NaiveDate::from_ymd_opt(2026, 1, 6).unwrap();

    let mut task = Task::new("blocked")
        .name("Blocked Task")
        .duration(Duration::days(5));
    task.complete = Some(50.0);
    // Even at 50%, mark as blocked
    task.status = Some(TaskStatus::Blocked);
    project.tasks.push(task);

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).expect("Scheduling should succeed");

    let blocked = schedule.tasks.get("blocked").expect("blocked task should exist");
    assert_eq!(blocked.status, TaskStatus::Blocked);
    assert_eq!(blocked.percent_complete, 50);
}
