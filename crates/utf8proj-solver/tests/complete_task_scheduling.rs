//! Tests for complete task scheduling edge cases
//!
//! This module tests the scheduling of 100% complete tasks, particularly
//! edge cases around actual_start and actual_finish dates.
//!
//! Bug fix (v0.15.1): Complete tasks without actual_start were incorrectly
//! locked to day 0, ignoring dependencies. This caused infeasible schedules
//! when sequential tasks were both 100% complete.

use chrono::NaiveDate;
use utf8proj_core::{Duration, Project, Resource, Scheduler, Task, TaskStatus};
use utf8proj_solver::CpmSolver;

/// Helper to create a date
fn date(year: i32, month: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(year, month, day).unwrap()
}

// =============================================================================
// Bug Regression Tests: Complete Tasks Without actual_start
// =============================================================================

/// Regression test for the original bug: two sequential 100% complete tasks
/// without actual_start dates should schedule correctly based on dependencies.
#[test]
fn complete_tasks_without_actual_start_respect_dependencies() {
    let mut project = Project::new("Bug Regression Test");
    project.start = date(2026, 1, 6);
    project.status_date = Some(date(2026, 1, 20));

    // Both tasks are 100% complete but have no actual_start dates
    let mut task_a = Task::new("task_a")
        .name("Task A")
        .duration(Duration::days(5));
    task_a.complete = Some(100.0);

    let mut task_b = Task::new("task_b")
        .name("Task B")
        .duration(Duration::days(5))
        .depends_on("task_a");
    task_b.complete = Some(100.0);

    project.tasks.push(task_a);
    project.tasks.push(task_b);

    let solver = CpmSolver::new();
    let schedule = solver
        .schedule(&project)
        .expect("Should schedule without error");

    let a = schedule.tasks.get("task_a").expect("task_a");
    let b = schedule.tasks.get("task_b").expect("task_b");

    // Task A should start at project start
    assert_eq!(a.forecast_start, date(2026, 1, 6));
    // Task B should start after Task A finishes
    assert!(
        b.forecast_start > a.forecast_finish,
        "Task B ({}) should start after Task A finishes ({})",
        b.forecast_start,
        a.forecast_finish
    );
    // Both should be complete
    assert_eq!(a.status, TaskStatus::Complete);
    assert_eq!(b.status, TaskStatus::Complete);
}

/// Three sequential complete tasks without actual dates
#[test]
fn three_sequential_complete_tasks_without_actual_dates() {
    let mut project = Project::new("Triple Chain Test");
    project.start = date(2026, 1, 6);

    let mut task_a = Task::new("a").duration(Duration::days(5));
    task_a.complete = Some(100.0);

    let mut task_b = Task::new("b").duration(Duration::days(5)).depends_on("a");
    task_b.complete = Some(100.0);

    let mut task_c = Task::new("c").duration(Duration::days(5)).depends_on("b");
    task_c.complete = Some(100.0);

    project.tasks.push(task_a);
    project.tasks.push(task_b);
    project.tasks.push(task_c);

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).expect("Should schedule");

    let a = schedule.tasks.get("a").unwrap();
    let b = schedule.tasks.get("b").unwrap();
    let c = schedule.tasks.get("c").unwrap();

    // Each task should start after the previous one finishes
    assert!(b.forecast_start > a.forecast_finish);
    assert!(c.forecast_start > b.forecast_finish);

    // All complete
    assert_eq!(a.remaining_duration.as_days() as i64, 0);
    assert_eq!(b.remaining_duration.as_days() as i64, 0);
    assert_eq!(c.remaining_duration.as_days() as i64, 0);
}

/// Complete tasks with same resource should still schedule sequentially
#[test]
fn complete_tasks_same_resource_schedule_correctly() {
    let mut project = Project::new("Same Resource Test");
    project.start = date(2026, 1, 6);
    project.status_date = Some(date(2026, 1, 27));

    project
        .resources
        .push(Resource::new("dev").name("Developer"));

    let mut task_a = Task::new("a").duration(Duration::days(5)).assign("dev");
    task_a.complete = Some(100.0);

    let mut task_b = Task::new("b")
        .duration(Duration::days(5))
        .depends_on("a")
        .assign("dev");
    task_b.complete = Some(100.0);

    let task_c = Task::new("c")
        .duration(Duration::days(10))
        .depends_on("b")
        .assign("dev");

    project.tasks.push(task_a);
    project.tasks.push(task_b);
    project.tasks.push(task_c);

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).expect("Should schedule");

    let a = schedule.tasks.get("a").unwrap();
    let b = schedule.tasks.get("b").unwrap();
    let c = schedule.tasks.get("c").unwrap();

    // Sequential chain
    assert!(b.forecast_start > a.forecast_finish);
    assert!(c.forecast_start > b.forecast_finish);
}

// =============================================================================
// Mixed Scenarios: Some with actual dates, some without
// =============================================================================

/// First task has actual dates, second task is complete without dates
#[test]
fn first_with_actuals_second_without() {
    let mut project = Project::new("Mixed Actuals Test");
    project.start = date(2026, 1, 6);

    let mut task_a = Task::new("a").duration(Duration::days(5));
    task_a.complete = Some(100.0);
    task_a.actual_start = Some(date(2026, 1, 6));
    task_a.actual_finish = Some(date(2026, 1, 10));

    let mut task_b = Task::new("b").duration(Duration::days(5)).depends_on("a");
    task_b.complete = Some(100.0);
    // No actual dates - should derive from predecessor

    project.tasks.push(task_a);
    project.tasks.push(task_b);

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).expect("Should schedule");

    let a = schedule.tasks.get("a").unwrap();
    let b = schedule.tasks.get("b").unwrap();

    // Task A locked to actuals
    assert_eq!(a.forecast_start, date(2026, 1, 6));
    assert_eq!(a.forecast_finish, date(2026, 1, 10));

    // Task B derived from A's finish
    assert!(b.forecast_start > a.forecast_finish);
}

/// First task without dates, second task has actual dates
#[test]
fn first_without_actuals_second_with() {
    let mut project = Project::new("Reverse Mixed Test");
    project.start = date(2026, 1, 6);

    let mut task_a = Task::new("a").duration(Duration::days(5));
    task_a.complete = Some(100.0);
    // No actual dates

    let mut task_b = Task::new("b").duration(Duration::days(5)).depends_on("a");
    task_b.complete = Some(100.0);
    task_b.actual_start = Some(date(2026, 1, 13));
    task_b.actual_finish = Some(date(2026, 1, 17));

    project.tasks.push(task_a);
    project.tasks.push(task_b);

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).expect("Should schedule");

    let a = schedule.tasks.get("a").unwrap();
    let b = schedule.tasks.get("b").unwrap();

    // Task A derived from project start
    assert_eq!(a.forecast_start, date(2026, 1, 6));

    // Task B locked to actuals
    assert_eq!(b.forecast_start, date(2026, 1, 13));
    assert_eq!(b.forecast_finish, date(2026, 1, 17));
}

// =============================================================================
// Parallel Complete Tasks
// =============================================================================

/// Two complete tasks in parallel (no dependency between them)
#[test]
fn parallel_complete_tasks_without_dates() {
    let mut project = Project::new("Parallel Complete Test");
    project.start = date(2026, 1, 6);

    let mut task_a = Task::new("a").duration(Duration::days(5));
    task_a.complete = Some(100.0);

    let mut task_b = Task::new("b").duration(Duration::days(10));
    task_b.complete = Some(100.0);

    // Task C depends on both A and B
    let task_c = Task::new("c")
        .duration(Duration::days(5))
        .depends_on("a")
        .depends_on("b");

    project.tasks.push(task_a);
    project.tasks.push(task_b);
    project.tasks.push(task_c);

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).expect("Should schedule");

    let a = schedule.tasks.get("a").unwrap();
    let b = schedule.tasks.get("b").unwrap();
    let c = schedule.tasks.get("c").unwrap();

    // Both A and B start at project start (parallel)
    assert_eq!(a.forecast_start, date(2026, 1, 6));
    assert_eq!(b.forecast_start, date(2026, 1, 6));

    // C starts after the later of A and B finishes
    let later_finish = a.forecast_finish.max(b.forecast_finish);
    assert!(c.forecast_start > later_finish);
}

/// Diamond dependency with all complete tasks
#[test]
fn diamond_dependency_all_complete() {
    let mut project = Project::new("Diamond Complete Test");
    project.start = date(2026, 1, 6);

    //     A
    //    / \
    //   B   C
    //    \ /
    //     D

    let mut a = Task::new("a").duration(Duration::days(5));
    a.complete = Some(100.0);

    let mut b = Task::new("b").duration(Duration::days(5)).depends_on("a");
    b.complete = Some(100.0);

    let mut c = Task::new("c").duration(Duration::days(10)).depends_on("a");
    c.complete = Some(100.0);

    let mut d = Task::new("d")
        .duration(Duration::days(5))
        .depends_on("b")
        .depends_on("c");
    d.complete = Some(100.0);

    project.tasks.push(a);
    project.tasks.push(b);
    project.tasks.push(c);
    project.tasks.push(d);

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).expect("Should schedule");

    let a_s = schedule.tasks.get("a").unwrap();
    let b_s = schedule.tasks.get("b").unwrap();
    let c_s = schedule.tasks.get("c").unwrap();
    let d_s = schedule.tasks.get("d").unwrap();

    // A starts at project start
    assert_eq!(a_s.forecast_start, date(2026, 1, 6));

    // B and C start after A
    assert!(b_s.forecast_start > a_s.forecast_finish);
    assert!(c_s.forecast_start > a_s.forecast_finish);

    // D starts after both B and C
    let later = b_s.forecast_finish.max(c_s.forecast_finish);
    assert!(d_s.forecast_start > later);
}

// =============================================================================
// Edge Cases: actual_finish only (no actual_start)
// =============================================================================

/// Task with actual_finish but no actual_start
#[test]
fn complete_with_actual_finish_only() {
    let mut project = Project::new("Finish Only Test");
    project.start = date(2026, 1, 6);

    let mut task = Task::new("a").duration(Duration::days(5));
    task.complete = Some(100.0);
    task.actual_finish = Some(date(2026, 1, 10));
    // No actual_start - should derive from predecessors (none) = project start

    project.tasks.push(task);

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).expect("Should schedule");

    let a = schedule.tasks.get("a").unwrap();

    // Should derive start, but lock to actual finish
    assert_eq!(a.forecast_finish, date(2026, 1, 10));
    assert_eq!(a.remaining_duration.as_days() as i64, 0);
}

/// Dependent task with actual_finish but no actual_start
#[test]
fn dependent_complete_with_actual_finish_only() {
    let mut project = Project::new("Dependent Finish Only Test");
    project.start = date(2026, 1, 6);

    let mut task_a = Task::new("a").duration(Duration::days(5));
    task_a.complete = Some(100.0);
    task_a.actual_start = Some(date(2026, 1, 6));
    task_a.actual_finish = Some(date(2026, 1, 10));

    let mut task_b = Task::new("b").duration(Duration::days(5)).depends_on("a");
    task_b.complete = Some(100.0);
    task_b.actual_finish = Some(date(2026, 1, 17));
    // No actual_start - should derive from A

    project.tasks.push(task_a);
    project.tasks.push(task_b);

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).expect("Should schedule");

    let a = schedule.tasks.get("a").unwrap();
    let b = schedule.tasks.get("b").unwrap();

    assert_eq!(a.forecast_finish, date(2026, 1, 10));
    // B starts after A, but locks to actual_finish
    assert!(b.forecast_start > a.forecast_finish);
    assert_eq!(b.forecast_finish, date(2026, 1, 17));
}

// =============================================================================
// Mixed Progress States with Complete Tasks
// =============================================================================

/// Complete → In-Progress → Not Started chain
#[test]
fn complete_inprogress_notstarted_chain() {
    let mut project = Project::new("Mixed Progress Chain");
    project.start = date(2026, 1, 6);
    project.status_date = Some(date(2026, 1, 20));

    let mut a = Task::new("a").duration(Duration::days(5));
    a.complete = Some(100.0);
    // No actual dates

    let mut b = Task::new("b").duration(Duration::days(10)).depends_on("a");
    b.complete = Some(50.0);
    b.actual_start = Some(date(2026, 1, 13));

    let c = Task::new("c").duration(Duration::days(5)).depends_on("b");

    project.tasks.push(a);
    project.tasks.push(b);
    project.tasks.push(c);

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).expect("Should schedule");

    let a_s = schedule.tasks.get("a").unwrap();
    let b_s = schedule.tasks.get("b").unwrap();
    let c_s = schedule.tasks.get("c").unwrap();

    assert_eq!(a_s.status, TaskStatus::Complete);
    assert_eq!(b_s.status, TaskStatus::InProgress);
    assert_eq!(c_s.status, TaskStatus::NotStarted);

    // Chain should be sequential
    assert!(b_s.forecast_start >= a_s.forecast_finish);
    assert!(c_s.forecast_start > b_s.forecast_finish);
}

/// Multiple complete predecessors converging on one task
#[test]
fn multiple_complete_predecessors() {
    let mut project = Project::new("Multi Predecessor Test");
    project.start = date(2026, 1, 6);

    let mut a = Task::new("a").duration(Duration::days(5));
    a.complete = Some(100.0);

    let mut b = Task::new("b").duration(Duration::days(10));
    b.complete = Some(100.0);

    let mut c = Task::new("c").duration(Duration::days(3));
    c.complete = Some(100.0);

    // D depends on all three
    let d = Task::new("d")
        .duration(Duration::days(5))
        .depends_on("a")
        .depends_on("b")
        .depends_on("c");

    project.tasks.push(a);
    project.tasks.push(b);
    project.tasks.push(c);
    project.tasks.push(d);

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).expect("Should schedule");

    let a_s = schedule.tasks.get("a").unwrap();
    let b_s = schedule.tasks.get("b").unwrap();
    let c_s = schedule.tasks.get("c").unwrap();
    let d_s = schedule.tasks.get("d").unwrap();

    // All start at project start (parallel)
    assert_eq!(a_s.forecast_start, date(2026, 1, 6));
    assert_eq!(b_s.forecast_start, date(2026, 1, 6));
    assert_eq!(c_s.forecast_start, date(2026, 1, 6));

    // D starts after the latest predecessor finishes
    let latest = a_s
        .forecast_finish
        .max(b_s.forecast_finish)
        .max(c_s.forecast_finish);
    assert!(d_s.forecast_start > latest);
}

// =============================================================================
// Containers with Complete Children
// =============================================================================

/// Container with all complete children (no actual dates)
#[test]
fn container_all_complete_children_no_dates() {
    let mut project = Project::new("Container Complete Test");
    project.start = date(2026, 1, 6);

    let mut child1 = Task::new("child1").duration(Duration::days(5));
    child1.complete = Some(100.0);

    let mut child2 = Task::new("child2")
        .duration(Duration::days(5))
        .depends_on("child1");
    child2.complete = Some(100.0);

    let container = Task::new("container")
        .name("Container")
        .child(child1)
        .child(child2);

    project.tasks.push(container);

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).expect("Should schedule");

    let c1 = schedule.tasks.get("container.child1").unwrap();
    let c2 = schedule.tasks.get("container.child2").unwrap();

    // child2 should start after child1
    assert!(c2.forecast_start > c1.forecast_finish);
}

// =============================================================================
// Slack Calculation with Complete Tasks
// =============================================================================

/// Complete tasks should still calculate correct slack
#[test]
fn complete_tasks_have_correct_slack() {
    let mut project = Project::new("Slack Test");
    project.start = date(2026, 1, 6);

    let mut a = Task::new("a").duration(Duration::days(5));
    a.complete = Some(100.0);

    let mut b = Task::new("b").duration(Duration::days(10)).depends_on("a");
    b.complete = Some(100.0);

    let c = Task::new("c").duration(Duration::days(5)).depends_on("a");

    let d = Task::new("d")
        .duration(Duration::days(3))
        .depends_on("b")
        .depends_on("c");

    project.tasks.push(a);
    project.tasks.push(b);
    project.tasks.push(c);
    project.tasks.push(d);

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).expect("Should schedule");

    // All tasks should have non-negative slack
    for (id, task) in &schedule.tasks {
        assert!(
            task.slack.as_days() >= 0.0,
            "Task {} has negative slack: {}",
            id,
            task.slack.as_days()
        );
    }

    // Critical path tasks (a -> b -> d) should have 0 slack
    // c should have positive slack (it's shorter than b)
    let c_s = schedule.tasks.get("c").unwrap();
    assert!(
        c_s.slack.as_days() > 0.0,
        "Task c should have positive slack"
    );
}

// =============================================================================
// Long Chains
// =============================================================================

/// Long chain of complete tasks (stress test)
#[test]
fn long_chain_complete_tasks() {
    let mut project = Project::new("Long Chain Test");
    project.start = date(2026, 1, 6);

    // Create a chain of 20 complete tasks
    for i in 0..20 {
        let mut task = Task::new(&format!("task_{}", i)).duration(Duration::days(1));
        task.complete = Some(100.0);
        if i > 0 {
            task = task.depends_on(&format!("task_{}", i - 1));
        }
        project.tasks.push(task);
    }

    let solver = CpmSolver::new();
    let schedule = solver
        .schedule(&project)
        .expect("Should schedule long chain");

    // Verify chain is sequential
    for i in 1..20 {
        let prev = schedule.tasks.get(&format!("task_{}", i - 1)).unwrap();
        let curr = schedule.tasks.get(&format!("task_{}", i)).unwrap();
        assert!(
            curr.forecast_start > prev.forecast_finish,
            "task_{} should start after task_{} finishes",
            i,
            i - 1
        );
    }
}
