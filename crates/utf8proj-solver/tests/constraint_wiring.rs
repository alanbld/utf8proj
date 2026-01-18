//! Tests for constraint wiring (Constraint Semantics v0)
//!
//! Phase 1: Forward pass constraints (floors + pins)
//! Phase 2: Backward pass constraints (ceilings + pins)
//! Phase 3: Global feasibility detection

use chrono::NaiveDate;
use utf8proj_core::{Duration, Project, Scheduler, Task, TaskConstraint};
use utf8proj_solver::CpmSolver;

fn date(year: i32, month: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(year, month, day).unwrap()
}

// =============================================================================
// Phase 1: Forward Pass Tests
// =============================================================================

#[test]
fn start_no_earlier_than_pushes_es() {
    // Task with no dependencies but StartNoEarlierThan constraint
    // Should start on constraint date, not project start
    let mut project = Project::new("SNET Test");
    project.start = date(2025, 1, 6); // Monday

    let mut task = Task::new("delayed").effort(Duration::days(5));
    task.constraints
        .push(TaskConstraint::StartNoEarlierThan(date(2025, 1, 13))); // Next Monday
    project.tasks.push(task);

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).unwrap();

    // Task should start on 2025-01-13, not 2025-01-06
    assert_eq!(
        schedule.tasks["delayed"].start,
        date(2025, 1, 13),
        "StartNoEarlierThan should push ES to constraint date"
    );
}

#[test]
fn start_no_earlier_than_respects_dependencies() {
    // Dependency pushes further than constraint - dependency wins
    let mut project = Project::new("SNET + Dep");
    project.start = date(2025, 1, 6);

    project
        .tasks
        .push(Task::new("first").effort(Duration::days(10))); // Finishes 2025-01-17

    let mut second = Task::new("second")
        .effort(Duration::days(5))
        .depends_on("first");
    second
        .constraints
        .push(TaskConstraint::StartNoEarlierThan(date(2025, 1, 13))); // Before dep finishes
    project.tasks.push(second);

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).unwrap();

    // Dependency finishes 2025-01-17, constraint is 2025-01-13
    // Task should start 2025-01-20 (first working day after dep)
    assert_eq!(
        schedule.tasks["second"].start,
        date(2025, 1, 20),
        "Dependency should push ES past StartNoEarlierThan"
    );
}

#[test]
fn finish_no_earlier_than_pushes_ef() {
    // Task must not finish before a certain date
    // If natural finish is earlier, ES must shift forward
    let mut project = Project::new("FNET Test");
    project.start = date(2025, 1, 6);

    let mut task = Task::new("held").effort(Duration::days(3));
    task.constraints
        .push(TaskConstraint::FinishNoEarlierThan(date(2025, 1, 17))); // Must finish on/after 1/17
    project.tasks.push(task);

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).unwrap();

    // 3-day task finishing on 1/17 would start on 1/15
    // Natural start would be 1/6, natural finish 1/8
    // Constraint should push to start 1/15, finish 1/17
    assert_eq!(
        schedule.tasks["held"].finish,
        date(2025, 1, 17),
        "FinishNoEarlierThan should push EF to constraint date"
    );
    assert_eq!(
        schedule.tasks["held"].start,
        date(2025, 1, 15),
        "FinishNoEarlierThan should shift ES accordingly"
    );
}

#[test]
fn must_finish_on_sets_dates() {
    // MustFinishOn pins the finish date, ES derives from it
    let mut project = Project::new("MFO Test");
    project.start = date(2025, 1, 6);

    let mut task = Task::new("pinned").effort(Duration::days(5));
    task.constraints
        .push(TaskConstraint::MustFinishOn(date(2025, 1, 24))); // Friday
    project.tasks.push(task);

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).unwrap();

    // 5-day task finishing 1/24 starts 1/20
    assert_eq!(
        schedule.tasks["pinned"].finish,
        date(2025, 1, 24),
        "MustFinishOn should pin finish date"
    );
    assert_eq!(
        schedule.tasks["pinned"].start,
        date(2025, 1, 20),
        "MustFinishOn should derive ES from EF - duration"
    );
}

#[test]
fn must_start_on_already_works() {
    // Verify existing MustStartOn still works (regression test)
    let mut project = Project::new("MSO Test");
    project.start = date(2025, 1, 6);

    let mut task = Task::new("pinned").effort(Duration::days(5));
    task.constraints
        .push(TaskConstraint::MustStartOn(date(2025, 1, 13)));
    project.tasks.push(task);

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).unwrap();

    assert_eq!(
        schedule.tasks["pinned"].start,
        date(2025, 1, 13),
        "MustStartOn should pin start date"
    );
}

// =============================================================================
// Phase 2: Backward Pass Tests
// =============================================================================

#[test]
fn start_no_later_than_caps_ls() {
    // Task with slack that gets reduced by StartNoLaterThan
    // Setup: predecessor (5d) -> task (3d), project has 15d worth of work
    // Without constraint: task has slack
    // With constraint: LS is capped
    let mut project = Project::new("SNLT Test");
    project.start = date(2025, 1, 6); // Monday

    let pred = Task::new("pred").effort(Duration::days(5));
    let mut task = Task::new("constrained")
        .effort(Duration::days(3))
        .depends_on("pred");
    task.constraints
        .push(TaskConstraint::StartNoLaterThan(date(2025, 1, 15))); // Wed of week 2
    let end_task = Task::new("end")
        .effort(Duration::days(5))
        .depends_on("constrained");

    project.tasks.push(pred);
    project.tasks.push(task);
    project.tasks.push(end_task);

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).unwrap();

    // ES of constrained = 5 (after pred), EF = 8
    // Without constraint, LS would be derived from "end" task needing it
    // With constraint StartNoLaterThan(1/15 = day 7), LS should be capped at 7
    assert!(
        schedule.tasks["constrained"].late_start <= date(2025, 1, 15),
        "StartNoLaterThan should cap LS at constraint date"
    );
}

#[test]
fn finish_no_later_than_caps_lf() {
    // Task with slack that gets reduced by FinishNoLaterThan
    let mut project = Project::new("FNLT Test");
    project.start = date(2025, 1, 6);

    let pred = Task::new("pred").effort(Duration::days(3));
    let mut task = Task::new("constrained")
        .effort(Duration::days(5))
        .depends_on("pred");
    task.constraints
        .push(TaskConstraint::FinishNoLaterThan(date(2025, 1, 17))); // Friday week 2
    let end_task = Task::new("end")
        .effort(Duration::days(5))
        .depends_on("constrained");

    project.tasks.push(pred);
    project.tasks.push(task);
    project.tasks.push(end_task);

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).unwrap();

    // LF should be capped at 2025-01-17
    assert!(
        schedule.tasks["constrained"].late_finish <= date(2025, 1, 17),
        "FinishNoLaterThan should cap LF at constraint date"
    );
}

#[test]
fn must_start_on_has_zero_slack() {
    // A pinned task should have zero slack
    let mut project = Project::new("Pin Slack");
    project.start = date(2025, 1, 6);

    let mut task = Task::new("pinned").effort(Duration::days(5));
    task.constraints
        .push(TaskConstraint::MustStartOn(date(2025, 1, 13)));
    project.tasks.push(task);

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).unwrap();

    assert_eq!(
        schedule.tasks["pinned"].slack,
        Duration::zero(),
        "Pinned task should have zero slack"
    );
    assert!(
        schedule.tasks["pinned"].is_critical,
        "Pinned task should be on critical path"
    );
}

#[test]
fn must_finish_on_has_zero_slack() {
    // A finish-pinned task should also have zero slack
    let mut project = Project::new("Finish Pin");
    project.start = date(2025, 1, 6);

    let mut task = Task::new("pinned").effort(Duration::days(5));
    task.constraints
        .push(TaskConstraint::MustFinishOn(date(2025, 1, 17)));
    project.tasks.push(task);

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).unwrap();

    assert_eq!(
        schedule.tasks["pinned"].slack,
        Duration::zero(),
        "Finish-pinned task should have zero slack"
    );
}

// =============================================================================
// Phase 3: Feasibility Detection Tests
// =============================================================================

#[test]
fn infeasible_constraint_dependency_conflict() {
    // MustStartOn date is before dependency can finish
    // This should fail with an error
    let mut project = Project::new("Infeasible");
    project.start = date(2025, 1, 6);

    project
        .tasks
        .push(Task::new("blocker").effort(Duration::days(10))); // Finishes 2025-01-17

    let mut blocked = Task::new("blocked")
        .effort(Duration::days(5))
        .depends_on("blocker");
    blocked
        .constraints
        .push(TaskConstraint::MustStartOn(date(2025, 1, 10))); // Before blocker finishes
    project.tasks.push(blocked);

    let solver = CpmSolver::new();
    let result = solver.schedule(&project);

    assert!(
        result.is_err(),
        "Should fail: MustStartOn conflicts with dependency"
    );
}

#[test]
fn infeasible_floor_ceiling_collapse() {
    // StartNoEarlierThan + StartNoLaterThan with impossible window
    let mut project = Project::new("Collapsed Window");
    project.start = date(2025, 1, 6);

    let mut task = Task::new("impossible").effort(Duration::days(5));
    task.constraints
        .push(TaskConstraint::StartNoEarlierThan(date(2025, 1, 20)));
    task.constraints
        .push(TaskConstraint::StartNoLaterThan(date(2025, 1, 10)));
    project.tasks.push(task);

    let solver = CpmSolver::new();
    let result = solver.schedule(&project);

    assert!(
        result.is_err(),
        "Should fail: floor > ceiling makes schedule infeasible"
    );
}

#[test]
fn infeasible_finish_before_start() {
    // FinishNoLaterThan is before StartNoEarlierThan + duration
    let mut project = Project::new("Impossible Duration");
    project.start = date(2025, 1, 6);

    let mut task = Task::new("squeezed").effort(Duration::days(10));
    task.constraints
        .push(TaskConstraint::StartNoEarlierThan(date(2025, 1, 13)));
    task.constraints
        .push(TaskConstraint::FinishNoLaterThan(date(2025, 1, 17))); // Only 5 days!
    project.tasks.push(task);

    let solver = CpmSolver::new();
    let result = solver.schedule(&project);

    assert!(
        result.is_err(),
        "Should fail: 10-day task cannot fit in 5-day window"
    );
}

#[test]
fn feasible_window_fits() {
    // Constraint window exactly fits task duration
    let mut project = Project::new("Tight Fit");
    project.start = date(2025, 1, 6);

    let mut task = Task::new("bounded").effort(Duration::days(5));
    task.constraints
        .push(TaskConstraint::StartNoEarlierThan(date(2025, 1, 13)));
    task.constraints
        .push(TaskConstraint::FinishNoLaterThan(date(2025, 1, 17)));
    project.tasks.push(task);

    let solver = CpmSolver::new();
    let result = solver.schedule(&project);

    assert!(
        result.is_ok(),
        "Should succeed: 5-day task fits in 5-day window"
    );

    let schedule = result.unwrap();
    assert_eq!(schedule.tasks["bounded"].start, date(2025, 1, 13));
    assert_eq!(schedule.tasks["bounded"].finish, date(2025, 1, 17));
    assert_eq!(schedule.tasks["bounded"].slack, Duration::zero());
}

// =============================================================================
// Non-Working Day Constraint Tests
// =============================================================================

#[test]
fn snet_on_sunday_rounds_forward_to_monday() {
    // Bug reproduction: SNET constraint on a non-working day should round FORWARD
    // to the next working day, not backward to the previous working day.
    //
    // Example: SNET = 2025-01-12 (Sunday)
    // Expected: task starts on 2025-01-13 (Monday)
    // Bug: task was starting on 2025-01-10 (Friday)
    let mut project = Project::new("SNET Sunday Test");
    project.start = date(2025, 1, 6); // Monday

    let mut task = Task::new("constrained").effort(Duration::days(5));
    task.constraints
        .push(TaskConstraint::StartNoEarlierThan(date(2025, 1, 12))); // Sunday!
    project.tasks.push(task);

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).unwrap();

    // Task should start on Monday 2025-01-13 (next working day after Sunday)
    // NOT Friday 2025-01-10 (previous working day)
    assert_eq!(
        schedule.tasks["constrained"].start,
        date(2025, 1, 13),
        "SNET on Sunday should round forward to Monday, not backward to Friday"
    );
}

#[test]
fn snet_milestone_on_weekend_stays_on_weekend() {
    // Milestones are events, not work - they can occur on any day
    // A release milestone on Saturday should stay on Saturday
    let mut project = Project::new("SNET Milestone Weekend");
    project.start = date(2025, 1, 6); // Monday

    let mut milestone = Task::new("milestone");
    milestone.milestone = true;
    milestone
        .constraints
        .push(TaskConstraint::StartNoEarlierThan(date(2025, 1, 11))); // Saturday!
    project.tasks.push(milestone);

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).unwrap();

    // Milestone should be on Saturday 2025-01-11 (exact constraint date)
    // Milestones ignore working day rules - they are events, not work
    assert_eq!(
        schedule.tasks["milestone"].start,
        date(2025, 1, 11),
        "SNET milestone on Saturday should stay on Saturday (events can occur any day)"
    );
}

#[test]
fn nested_milestone_snet_on_weekend_stays_on_weekend() {
    // Milestones are events, not work - they can occur on any day
    // A nested milestone (e.g., Sunday release) should stay on Sunday
    let mut project = Project::new("Nested SNET");
    project.start = date(2025, 1, 6); // Monday

    // Container with a nested milestone
    let mut container = Task::new("container");
    container.name = "Container".to_string();

    let mut child = Task::new("child").effort(Duration::days(3));
    child.name = "Child".to_string();

    let mut milestone = Task::new("release");
    milestone.name = "Release".to_string();
    milestone.milestone = true;
    milestone
        .constraints
        .push(TaskConstraint::StartNoEarlierThan(date(2025, 1, 19))); // Sunday!

    container.children.push(child);
    container.children.push(milestone);
    project.tasks.push(container);

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).unwrap();

    // Nested milestone should be on Sunday 2025-01-19 (exact constraint date)
    // Milestones ignore working day rules - they are events, not work
    assert_eq!(
        schedule.tasks["container.release"].start,
        date(2025, 1, 19),
        "Nested milestone SNET on Sunday should stay on Sunday (events can occur any day)"
    );
}
