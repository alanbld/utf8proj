//! RFC-0004: Progress-Aware CPM - Acceptance Tests (Golden Fixtures)
//!
//! These 8 tests define the expected behavior for Progress-Aware CPM.
//! All tests are marked #[ignore] until the required APIs are implemented.
//!
//! ## Required API Changes (Implementation Checklist)
//!
//! ### utf8proj-core
//! - [ ] Add `Project.status_date: Option<NaiveDate>` field
//! - [ ] Add `Task.explicit_remaining: Option<Duration>` field
//! - [ ] Add `DiagnosticCode::P005RemainingCompleteConflict`
//! - [ ] Add `DiagnosticCode::P006ContainerCompleteOverride`
//!
//! ### utf8proj-solver
//! - [ ] Add `CpmSolver::with_status_date(date: NaiveDate)` constructor
//! - [ ] Modify forward pass to respect progress data
//! - [ ] Schedule remaining work from status_date, not project.start
//! - [ ] Lock completed tasks to actual dates
//!
//! ### utf8proj-parser
//! - [ ] Parse `status_date: YYYY-MM-DD` in project block
//! - [ ] Parse `remaining: Nd` on tasks
//!
//! ### utf8proj-cli
//! - [ ] Add `--as-of YYYY-MM-DD` flag (overrides project.status_date)
//!
//! ## Design Clarifications (Locked)
//!
//! - C-01: status_date resolution: --as-of CLI > project.status_date > today()
//! - C-02: remaining_duration precedence: complete=100% → 0, explicit remaining → use it, else linear
//! - C-03: Explicit container.complete overrides derived (P006 if differs >10%)
//! - C-04: Baseline stores per-task ES/EF internally, grammar exposes only project.finish in Phase 1

use chrono::NaiveDate;
use utf8proj_core::{Duration, Project, Scheduler, Task, TaskStatus};
use utf8proj_solver::CpmSolver;

/// Helper to create a date
fn date(year: i32, month: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(year, month, day).unwrap()
}

// =============================================================================
// Test 1: Completed Task Locks to Actual Dates
// =============================================================================
/// A task at 100% complete should use actual_start and actual_finish as its
/// scheduled dates, regardless of what the forward pass would compute.
/// Remaining duration = 0.
///
/// NOTE: This test PASSES with current implementation - completed tasks already
/// lock to actual_finish. The forward pass respects actual dates for complete tasks.
#[test]
fn test_01_completed_task_locks_to_actual_dates() {
    let mut project = Project::new("Completed Task Test");
    project.start = date(2026, 1, 6);
    // TODO: Uncomment when Project.status_date is added
    // project.status_date = Some(date(2026, 1, 20));

    // Task completed early (actual_finish before baseline would have finished)
    let mut task = Task::new("design")
        .name("Design Phase")
        .duration(Duration::days(10));
    task.complete = Some(100.0);
    task.actual_start = Some(date(2026, 1, 6));
    task.actual_finish = Some(date(2026, 1, 15)); // Finished on day 8, not day 10

    project.tasks.push(task);

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).expect("Should schedule");

    let design = schedule.tasks.get("design").expect("design exists");

    // Completed task uses actual dates
    assert_eq!(design.forecast_start, date(2026, 1, 6), "forecast_start = actual_start");
    assert_eq!(design.forecast_finish, date(2026, 1, 15), "forecast_finish = actual_finish");
    assert_eq!(design.remaining_duration.as_days() as i64, 0, "remaining = 0");
    assert_eq!(design.percent_complete, 100);
    assert_eq!(design.status, TaskStatus::Complete);
}

// =============================================================================
// Test 2: Partial Progress with Remaining Duration
// =============================================================================
/// A task in progress should schedule remaining work from status_date.
/// If remaining_duration is explicit, use it. Otherwise derive linearly.
///
/// REQUIRES:
/// - Project.status_date field
/// - Forward pass schedules remaining work from status_date
#[test]
fn test_02_partial_progress_schedules_from_status_date() {
    let mut project = Project::new("Partial Progress Test");
    project.start = date(2026, 1, 6);
    project.status_date = Some(date(2026, 1, 13)); // Tuesday (Jan 1, 2026 = Thu) (RFC-0004)

    // 10-day task at 50% complete on day 5
    let mut task = Task::new("develop")
        .name("Development")
        .duration(Duration::days(10));
    task.complete = Some(50.0);
    task.actual_start = Some(date(2026, 1, 6));
    // remaining_duration not set → derive linearly: 10 * (1 - 0.5) = 5 days

    project.tasks.push(task);

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).expect("Should schedule");

    let develop = schedule.tasks.get("develop").expect("develop exists");

    // Started, so forecast_start = actual_start
    assert_eq!(develop.forecast_start, date(2026, 1, 6), "forecast_start = actual_start");
    // Remaining 5 days from status_date (2026-01-13)
    // In 2026: Jan 13=Tue, so 5 working days = Tue,Wed,Thu,Fri,Mon = Jan 19
    assert_eq!(develop.remaining_duration.as_days() as i64, 5, "remaining = 5 days (linear)");
    assert_eq!(develop.forecast_finish, date(2026, 1, 19), "forecast_finish = status_date + 5 working days");
    assert_eq!(develop.percent_complete, 50);
    assert_eq!(develop.status, TaskStatus::InProgress);
}

/// Explicit remaining_duration takes precedence over linear derivation
///
/// REQUIRES:
/// - Task.explicit_remaining field
/// - Solver respects explicit remaining over linear calculation
#[test]
fn test_02b_explicit_remaining_duration_precedence() {
    let mut project = Project::new("Explicit Remaining Test");
    project.start = date(2026, 1, 6);
    project.status_date = Some(date(2026, 1, 13));

    // 10-day task at 50% complete, but explicit remaining = 8 days
    // (indicates task is behind - work took longer than planned)
    let mut task = Task::new("develop")
        .name("Development")
        .duration(Duration::days(10));
    task.complete = Some(50.0);
    task.actual_start = Some(date(2026, 1, 6));
    task.explicit_remaining = Some(Duration::days(8)); // Explicit: 8 days remaining

    project.tasks.push(task);

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).expect("Should schedule");

    let develop = schedule.tasks.get("develop").expect("develop exists");

    // Explicit remaining takes precedence
    assert_eq!(develop.remaining_duration.as_days() as i64, 8, "remaining = explicit 8 days");
    // 8 working days from Mon Jan 13 = Wed Jan 22
    assert_eq!(develop.forecast_finish, date(2026, 1, 22), "forecast_finish = status_date + 8 working days");
}

// =============================================================================
// Test 3: Future Tasks Schedule Normally from Predecessors
// =============================================================================
/// Tasks not yet started schedule normally using forward pass from predecessors.
/// status_date affects the "now" reference but not unstarted task scheduling.
///
/// REQUIRES:
/// - Project.status_date field
/// - Progress-aware forward pass for in-progress predecessors
#[test]
fn test_03_future_tasks_schedule_from_predecessors() {
    let mut project = Project::new("Future Tasks Test");
    project.start = date(2026, 1, 6);
    project.status_date = Some(date(2026, 1, 16)); // Friday (Jan 16 is Fri in 2026) (RFC-0004)

    // Task A: completed
    let mut task_a = Task::new("task_a")
        .name("Task A")
        .duration(Duration::days(5));
    task_a.complete = Some(100.0);
    task_a.actual_start = Some(date(2026, 1, 6));
    task_a.actual_finish = Some(date(2026, 1, 12)); // Mon Jan 12 (5 working days from Tue Jan 6)
    project.tasks.push(task_a);

    // Task B: in progress
    let mut task_b = Task::new("task_b")
        .name("Task B")
        .duration(Duration::days(5))
        .depends_on("task_a");
    task_b.complete = Some(60.0);
    task_b.actual_start = Some(date(2026, 1, 13)); // Tue Jan 13
    // remaining = ceil(5 * 0.4) = 2 days
    project.tasks.push(task_b);

    // Task C: not started, depends on B
    let task_c = Task::new("task_c")
        .name("Task C")
        .duration(Duration::days(5))
        .depends_on("task_b");
    project.tasks.push(task_c);

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).expect("Should schedule");

    let task_c_sched = schedule.tasks.get("task_c").expect("task_c exists");

    // In 2026: Jan 16 is Fri, status_date_days = 8 (8 working days from Jan 6)
    // Task B: remaining = 2 days, EF = 8 + 2 = 10
    // Task B finishes: working day 9 = Jan 19 (Mon, skip Jan 17-18 weekend)
    // Task C starts: working day 10 = Jan 20
    // Task C: 5 days, EF = 10 + 5 = 15
    // Task C finishes: working day 14 = Jan 26 (skip Jan 24-25 weekend)
    assert_eq!(task_c_sched.status, TaskStatus::NotStarted);
    assert_eq!(task_c_sched.forecast_start, date(2026, 1, 20), "task_c starts after task_b finishes");
    assert_eq!(task_c_sched.forecast_finish, date(2026, 1, 26), "task_c = 5 working days");
    assert_eq!(task_c_sched.remaining_duration.as_days() as i64, 5, "remaining = full duration");
}

// =============================================================================
// Test 4: Dependency Chain with Mixed Progress States
// =============================================================================
/// Complex chain: completed → in-progress → not-started → not-started
/// Each segment uses the appropriate scheduling rule.
///
/// REQUIRES:
/// - All progress-aware forward pass features
#[test]
fn test_04_dependency_chain_mixed_progress() {
    let mut project = Project::new("Dependency Chain Test");
    project.start = date(2026, 1, 6);
    project.status_date = Some(date(2026, 1, 20)); // Tuesday week 3

    // Phase 1: Design (complete)
    let mut design = Task::new("design")
        .name("Design")
        .duration(Duration::days(5));
    design.complete = Some(100.0);
    design.actual_start = Some(date(2026, 1, 6));
    design.actual_finish = Some(date(2026, 1, 10));
    project.tasks.push(design);

    // Phase 2: Develop (in progress, 40% done)
    let mut develop = Task::new("develop")
        .name("Development")
        .duration(Duration::days(10))
        .depends_on("design");
    develop.complete = Some(40.0);
    develop.actual_start = Some(date(2026, 1, 13));
    // remaining = 10 * 0.6 = 6 days
    project.tasks.push(develop);

    // Phase 3: Test (not started)
    let test = Task::new("test")
        .name("Testing")
        .duration(Duration::days(5))
        .depends_on("develop");
    project.tasks.push(test);

    // Phase 4: Deploy (not started)
    let deploy = Task::new("deploy")
        .name("Deployment")
        .duration(Duration::days(3))
        .depends_on("test");
    project.tasks.push(deploy);

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).expect("Should schedule");

    // Design: locked to actuals
    let design_s = schedule.tasks.get("design").expect("design");
    assert_eq!(design_s.forecast_finish, date(2026, 1, 10));

    // Develop: status_date(Jan 20) + 6 remaining days = Mon Jan 27
    let develop_s = schedule.tasks.get("develop").expect("develop");
    assert_eq!(develop_s.remaining_duration.as_days() as i64, 6);
    assert_eq!(develop_s.forecast_finish, date(2026, 1, 27));

    // Test: starts Jan 28, 5 days → Feb 3
    let test_s = schedule.tasks.get("test").expect("test");
    assert_eq!(test_s.forecast_start, date(2026, 1, 28));
    assert_eq!(test_s.forecast_finish, date(2026, 2, 3));

    // Deploy: starts Feb 4, 3 days → Feb 6
    let deploy_s = schedule.tasks.get("deploy").expect("deploy");
    assert_eq!(deploy_s.forecast_start, date(2026, 2, 4));
    assert_eq!(deploy_s.forecast_finish, date(2026, 2, 6));
}

// =============================================================================
// Test 5: Container Weighted Rollup
// =============================================================================
/// Container progress = Σ(child.duration × child.complete) / Σ(child.duration)
/// This weighted average reflects earned value properly.
///
/// NOTE: This test may already pass - container rollup exists in progress_tracking.rs
#[test]
fn test_05_container_weighted_rollup() {
    let mut project = Project::new("Container Rollup Test");
    project.start = date(2026, 1, 6);
    // TODO: project.status_date = Some(date(2026, 1, 20));

    // Container with children at different progress:
    // - backend: 20d @ 75% = 15 earned days
    // - frontend: 10d @ 30% = 3 earned days
    // - testing: 10d @ 0% = 0 earned days
    // Total: 40d, earned: 18d, container % = 18/40 = 45%
    let mut backend = Task::new("backend")
        .name("Backend")
        .duration(Duration::days(20));
    backend.complete = Some(75.0);
    backend.actual_start = Some(date(2026, 1, 6));

    let mut frontend = Task::new("frontend")
        .name("Frontend")
        .duration(Duration::days(10));
    frontend.complete = Some(30.0);
    frontend.actual_start = Some(date(2026, 1, 6));

    let testing = Task::new("testing")
        .name("Testing")
        .duration(Duration::days(10));

    let container = Task::new("development")
        .name("Development Phase")
        .child(backend)
        .child(frontend)
        .child(testing);

    project.tasks.push(container);

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).expect("Should schedule");

    let dev = schedule.tasks.get("development").expect("development");

    // Container derives progress from weighted average
    assert_eq!(dev.percent_complete, 45, "container = 45% (weighted average)");
    assert_eq!(dev.status, TaskStatus::InProgress);
}

// =============================================================================
// Test 6: status_date Resolution Order
// =============================================================================
/// C-01: --as-of CLI > project.status_date > today()
/// This test verifies project.status_date is used when set.
///
/// NOTE: Phase 1 (status_date infrastructure) complete.
/// Test still ignored because Phase 2 (progress-aware forward pass) not implemented.
#[test]
fn test_06_status_date_resolution() {
    let mut project = Project::new("Status Date Test");
    project.start = date(2026, 1, 6);
    project.status_date = Some(date(2026, 1, 27)); // Explicit status date (Tue in 2026)

    // Task at 50% with 10-day duration
    let mut task = Task::new("work")
        .name("Work")
        .duration(Duration::days(10));
    task.complete = Some(50.0);
    task.actual_start = Some(date(2026, 1, 6));
    project.tasks.push(task);

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).expect("Should schedule");

    let work = schedule.tasks.get("work").expect("work");

    // remaining = 5 days, scheduled from status_date (Jan 27)
    // In 2026: Jan 27=Tue, so 5 working days = Tue,Wed,Thu,Fri,Mon = Feb 2
    // (skip Jan 31-Feb 1 weekend)
    assert_eq!(work.remaining_duration.as_days() as i64, 5);
    assert_eq!(work.forecast_finish, date(2026, 2, 2), "finish = status_date + remaining");
}

/// Test CLI --as-of override
///
/// NOTE: Phase 1 complete - CpmSolver::with_status_date() and effective_status_date() exist.
/// Test still ignored because Phase 2 (progress-aware forward pass) not implemented.
#[test]
fn test_06b_cli_as_of_override() {
    let mut project = Project::new("CLI Override Test");
    project.start = date(2026, 1, 6);
    project.status_date = Some(date(2026, 1, 27)); // Project says Jan 27

    let mut task = Task::new("work")
        .name("Work")
        .duration(Duration::days(10));
    task.complete = Some(50.0);
    task.actual_start = Some(date(2026, 1, 6));
    project.tasks.push(task);

    // CLI override: --as-of 2026-01-20 should take precedence
    let solver = CpmSolver::with_status_date(date(2026, 1, 20));
    let schedule = solver.schedule(&project).expect("Should schedule");

    let work = schedule.tasks.get("work").expect("work");

    // Should use CLI date (Jan 20=Tue), not project date (Jan 27)
    // In 2026: 5 remaining days from Jan 20 = Tue,Wed,Thu,Fri,Mon = Jan 26
    // (skip Jan 24-25 weekend)
    assert_eq!(work.forecast_finish, date(2026, 1, 26), "CLI --as-of overrides project.status_date");
}

// =============================================================================
// Test 7: remaining_duration vs complete Conflict → P005
// =============================================================================
/// C-02: When remaining_duration and complete are inconsistent, emit P005 diagnostic
/// and use the precedence: complete=100% → 0, explicit remaining → use it
///
/// REQUIRES:
/// - Task.explicit_remaining field
/// - DiagnosticCode::P005RemainingCompleteConflict
#[test]
fn test_07_remaining_vs_complete_conflict_p005() {
    use utf8proj_core::{CollectingEmitter, DiagnosticCode};
    use utf8proj_solver::{analyze_project, AnalysisConfig};

    let mut project = Project::new("Conflict Test");
    project.start = date(2026, 1, 6);
    project.status_date = Some(date(2026, 1, 13));

    // Task at 50% complete but explicit remaining = 8 days
    // Linear derivation would be 5 days, so this is inconsistent
    let mut task = Task::new("work")
        .name("Work")
        .duration(Duration::days(10));
    task.complete = Some(50.0);
    task.actual_start = Some(date(2026, 1, 6));
    task.explicit_remaining = Some(Duration::days(8)); // Inconsistent with 50%
    project.tasks.push(task);

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).expect("Should schedule");

    // P005 diagnostic should be emitted
    let mut emitter = CollectingEmitter::new();
    let config = AnalysisConfig::default();
    analyze_project(&project, Some(&schedule), &config, &mut emitter);

    assert!(
        emitter
            .diagnostics
            .iter()
            .any(|d| d.code == DiagnosticCode::P005RemainingCompleteConflict),
        "Should emit P005 for remaining vs complete conflict"
    );

    // Explicit remaining takes precedence
    let work = schedule.tasks.get("work").expect("work");
    assert_eq!(work.remaining_duration.as_days() as i64, 8, "explicit remaining wins");
}

/// complete=100% always means remaining=0, even if explicit remaining disagrees
///
/// NOTE: This test PASSES with current implementation - the solver already
/// derives remaining=0 when complete=100%.
#[test]
fn test_07b_complete_100_forces_remaining_zero() {
    let mut project = Project::new("Complete Forces Zero Test");
    project.start = date(2026, 1, 6);
    // TODO: project.status_date = Some(date(2026, 1, 20));

    // Task at 100% complete but explicit remaining = 5 days (impossible)
    let mut task = Task::new("done")
        .name("Done")
        .duration(Duration::days(10));
    task.complete = Some(100.0);
    task.actual_start = Some(date(2026, 1, 6));
    task.actual_finish = Some(date(2026, 1, 17));
    // TODO: task.explicit_remaining = Some(Duration::days(5)); // Contradicts 100%
    project.tasks.push(task);

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).expect("Should schedule");

    let done = schedule.tasks.get("done").expect("done");

    // 100% complete always means remaining = 0
    assert_eq!(done.remaining_duration.as_days() as i64, 0, "100% complete forces remaining=0");
    assert_eq!(done.status, TaskStatus::Complete);
}

// =============================================================================
// Test 8: Container Explicit complete Override → P006
// =============================================================================
/// C-03: Explicit container.complete overrides derived value.
/// If explicit differs from derived by >10%, emit P006 warning.
///
/// REQUIRES:
/// - DiagnosticCode::P006ContainerProgressMismatch
/// - analyze_project emits P006 when container explicit differs from derived
#[test]
fn test_08_container_explicit_complete_p006() {
    use utf8proj_core::{CollectingEmitter, DiagnosticCode};
    use utf8proj_solver::{analyze_project, AnalysisConfig};

    let mut project = Project::new("Container Override Test");
    project.start = date(2026, 1, 6);
    project.status_date = Some(date(2026, 1, 20));

    // Children: 50% average, but container claims 80%
    let mut backend = Task::new("backend")
        .name("Backend")
        .duration(Duration::days(10));
    backend.complete = Some(50.0);

    let mut frontend = Task::new("frontend")
        .name("Frontend")
        .duration(Duration::days(10));
    frontend.complete = Some(50.0);

    let mut container = Task::new("development")
        .name("Development");
    container = container.child(backend).child(frontend);
    // Explicit override: container says 80% but children average 50%
    container.complete = Some(80.0);

    project.tasks.push(container);

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).expect("Should schedule");

    // P006 diagnostic should be emitted (80% explicit vs 50% derived = 30% diff > 10%)
    let mut emitter = CollectingEmitter::new();
    let config = AnalysisConfig::default();
    analyze_project(&project, Some(&schedule), &config, &mut emitter);

    assert!(
        emitter
            .diagnostics
            .iter()
            .any(|d| d.code == DiagnosticCode::P006ContainerProgressMismatch),
        "Should emit P006 for container complete override (diff > 10%)"
    );

    // Explicit complete is used despite being inconsistent
    let dev = schedule.tasks.get("development").expect("development");
    assert_eq!(dev.percent_complete, 80, "explicit complete overrides derived");
}

/// No P006 if explicit is within 10% of derived
///
/// REQUIRES:
/// - P006 threshold check (10% difference)
#[test]
fn test_08b_container_explicit_within_threshold_no_p006() {
    use utf8proj_core::{CollectingEmitter, DiagnosticCode};
    use utf8proj_solver::{analyze_project, AnalysisConfig};

    let mut project = Project::new("Container Threshold Test");
    project.start = date(2026, 1, 6);
    project.status_date = Some(date(2026, 1, 20));

    // Children: 50% average, container claims 55% (within 10%)
    let mut backend = Task::new("backend")
        .name("Backend")
        .duration(Duration::days(10));
    backend.complete = Some(50.0);

    let mut frontend = Task::new("frontend")
        .name("Frontend")
        .duration(Duration::days(10));
    frontend.complete = Some(50.0);

    let mut container = Task::new("development")
        .name("Development");
    container = container.child(backend).child(frontend);
    container.complete = Some(55.0); // Within 10% of 50%

    project.tasks.push(container);

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).expect("Should schedule");

    let mut emitter = CollectingEmitter::new();
    let config = AnalysisConfig::default();
    analyze_project(&project, Some(&schedule), &config, &mut emitter);

    // No P006 - difference is within threshold
    assert!(
        !emitter
            .diagnostics
            .iter()
            .any(|d| d.code == DiagnosticCode::P006ContainerProgressMismatch),
        "Should NOT emit P006 when explicit is within 10% of derived"
    );
}
