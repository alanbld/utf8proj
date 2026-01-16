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
    let schedule = solver
        .schedule(&project)
        .expect("Scheduling should succeed");

    // Check design task progress
    let design = schedule
        .tasks
        .get("design")
        .expect("design task should exist");
    assert_eq!(design.percent_complete, 60);
    assert_eq!(design.status, TaskStatus::InProgress);
    assert_eq!(design.remaining_duration.as_days() as i64, 4); // 40% of 10 days

    // Check implement task progress
    let implement = schedule
        .tasks
        .get("implement")
        .expect("implement task should exist");
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
    let schedule = solver
        .schedule(&project)
        .expect("Scheduling should succeed");

    let done = schedule.tasks.get("done").expect("done task should exist");
    assert_eq!(done.percent_complete, 100);
    assert_eq!(done.status, TaskStatus::Complete);
    assert_eq!(done.remaining_duration.as_days() as i64, 0);
    assert_eq!(
        done.forecast_finish,
        NaiveDate::from_ymd_opt(2026, 1, 10).unwrap()
    );
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
    let schedule = solver
        .schedule(&project)
        .expect("Scheduling should succeed");

    let started = schedule
        .tasks
        .get("started")
        .expect("started task should exist");
    // Forecast start should use actual_start
    assert_eq!(
        started.forecast_start,
        NaiveDate::from_ymd_opt(2026, 1, 13).unwrap()
    );
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
    let schedule = solver
        .schedule(&project)
        .expect("Scheduling should succeed");

    let blocked = schedule
        .tasks
        .get("blocked")
        .expect("blocked task should exist");
    assert_eq!(blocked.status, TaskStatus::Blocked);
    assert_eq!(blocked.percent_complete, 50);
}

/// Test that container progress is derived from children (weighted average by duration)
#[test]
fn container_progress_derived_from_children() {
    let mut project = Project::new("Container Progress Test");
    project.start = NaiveDate::from_ymd_opt(2026, 1, 6).unwrap();

    // Container with children at different progress levels:
    // - backend: 20d @ 75% complete
    // - frontend: 10d @ 30% complete
    // - testing: 10d @ 0% complete
    // Expected container progress: (20*75 + 10*30 + 10*0) / (20+10+10) = 1800/40 = 45%
    let container = Task::new("development")
        .child(
            Task::new("backend")
                .duration(Duration::days(20))
                .complete(75.0),
        )
        .child(
            Task::new("frontend")
                .duration(Duration::days(10))
                .complete(30.0),
        )
        .child(Task::new("testing").duration(Duration::days(10)));
    project.tasks.push(container);

    let solver = CpmSolver::new();
    let schedule = solver
        .schedule(&project)
        .expect("Scheduling should succeed");

    // Check container progress is derived from children
    let dev = schedule
        .tasks
        .get("development")
        .expect("development container should exist");
    assert_eq!(
        dev.percent_complete, 45,
        "Container should derive 45% progress from weighted average of children"
    );
    assert_eq!(
        dev.status,
        TaskStatus::InProgress,
        "Container should be InProgress since children have progress"
    );

    // Check individual children have correct progress
    let backend = schedule
        .tasks
        .get("development.backend")
        .expect("backend task should exist");
    assert_eq!(backend.percent_complete, 75);
    assert_eq!(backend.status, TaskStatus::InProgress);

    let frontend = schedule
        .tasks
        .get("development.frontend")
        .expect("frontend task should exist");
    assert_eq!(frontend.percent_complete, 30);
    assert_eq!(frontend.status, TaskStatus::InProgress);

    let testing = schedule
        .tasks
        .get("development.testing")
        .expect("testing task should exist");
    assert_eq!(testing.percent_complete, 0);
    assert_eq!(testing.status, TaskStatus::NotStarted);
}

/// Test nested container progress (multi-level hierarchy)
#[test]
fn nested_container_progress() {
    let mut project = Project::new("Nested Container Test");
    project.start = NaiveDate::from_ymd_opt(2026, 1, 6).unwrap();

    // Nested structure:
    // project (container)
    // ├── phase1 (container)
    // │   ├── task_a: 10d @ 100%
    // │   └── task_b: 10d @ 50%
    // └── phase2 (container)
    //     └── task_c: 20d @ 0%
    //
    // Phase1: (10*100 + 10*50) / 20 = 75%
    // Project: (10*100 + 10*50 + 20*0) / 40 = 1500/40 = 37.5% ≈ 38%
    let root = Task::new("project")
        .child(
            Task::new("phase1")
                .child(
                    Task::new("task_a")
                        .duration(Duration::days(10))
                        .complete(100.0),
                )
                .child(
                    Task::new("task_b")
                        .duration(Duration::days(10))
                        .complete(50.0),
                ),
        )
        .child(Task::new("phase2").child(Task::new("task_c").duration(Duration::days(20))));
    project.tasks.push(root);

    let solver = CpmSolver::new();
    let schedule = solver
        .schedule(&project)
        .expect("Scheduling should succeed");

    // Check nested container progress
    let phase1 = schedule
        .tasks
        .get("project.phase1")
        .expect("phase1 should exist");
    assert_eq!(
        phase1.percent_complete, 75,
        "Phase1 should be 75% (weighted avg of task_a and task_b)"
    );
    assert_eq!(phase1.status, TaskStatus::InProgress);

    let phase2 = schedule
        .tasks
        .get("project.phase2")
        .expect("phase2 should exist");
    assert_eq!(
        phase2.percent_complete, 0,
        "Phase2 should be 0% (only task_c at 0%)"
    );
    assert_eq!(phase2.status, TaskStatus::NotStarted);

    // Top-level container gets flattened weighted average from all leaves
    let proj = schedule.tasks.get("project").expect("project should exist");
    assert_eq!(
        proj.percent_complete, 38,
        "Project should be 38% (weighted avg of all leaves)"
    );
    assert_eq!(proj.status, TaskStatus::InProgress);
}

/// Test container with all children complete shows 100%
#[test]
fn container_complete_when_all_children_complete() {
    let mut project = Project::new("Complete Container Test");
    project.start = NaiveDate::from_ymd_opt(2026, 1, 6).unwrap();

    let container = Task::new("done")
        .child(Task::new("a").duration(Duration::days(5)).complete(100.0))
        .child(Task::new("b").duration(Duration::days(5)).complete(100.0));
    project.tasks.push(container);

    let solver = CpmSolver::new();
    let schedule = solver
        .schedule(&project)
        .expect("Scheduling should succeed");

    let done = schedule
        .tasks
        .get("done")
        .expect("done container should exist");
    assert_eq!(done.percent_complete, 100);
    assert_eq!(done.status, TaskStatus::Complete);
}
