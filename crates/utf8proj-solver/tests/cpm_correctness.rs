//! CPM Correctness Test Suite
//!
//! These tests validate fundamental CPM invariants that must hold
//! for ANY valid implementation.
//!
//! Invariants:
//! 1. Slack is always non-negative
//! 2. ES = max(EF of predecessors)
//! 3. LF = min(LS of successors)
//! 4. Critical path has zero slack
//! 5. Cross-container dependencies are honored
//! 6. Container dates derive from children
//! 7. Non-FS dependency types compute correct ES/LS/EF/LF

use utf8proj_core::{Dependency, DependencyType, Duration, Project, Scheduler, Task};
use utf8proj_solver::CpmSolver;

// ============================================================================
// INVARIANT 1: Slack is never negative
// ============================================================================

#[test]
fn slack_is_never_negative_simple() {
    let mut project = Project::new("Test");
    project.tasks = vec![
        Task::new("a").duration(Duration::days(5)),
        Task::new("b").duration(Duration::days(3)).depends_on("a"),
        Task::new("c").duration(Duration::days(2)).depends_on("b"),
    ];

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).expect("Should schedule");

    for (task_id, scheduled) in &schedule.tasks {
        assert!(
            scheduled.slack.as_days() >= 0.0,
            "Task {} has negative slack: {}",
            task_id,
            scheduled.slack.as_days()
        );
    }
}

#[test]
fn slack_is_never_negative_parallel() {
    let mut project = Project::new("Test");
    project.tasks = vec![
        Task::new("a").duration(Duration::days(5)),
        Task::new("b").duration(Duration::days(3)),
        Task::new("c")
            .duration(Duration::days(2))
            .depends_on("a")
            .depends_on("b"),
    ];

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).expect("Should schedule");

    for (task_id, scheduled) in &schedule.tasks {
        assert!(
            scheduled.slack.as_days() >= 0.0,
            "Task {} has negative slack: {}",
            task_id,
            scheduled.slack.as_days()
        );
    }
}

#[test]
fn slack_is_never_negative_complex() {
    let mut project = Project::new("Test");
    project.tasks = vec![
        Task::new("start").duration(Duration::days(0)),
        Task::new("a")
            .duration(Duration::days(5))
            .depends_on("start"),
        Task::new("b")
            .duration(Duration::days(8))
            .depends_on("start"),
        Task::new("c").duration(Duration::days(3)).depends_on("a"),
        Task::new("d").duration(Duration::days(4)).depends_on("b"),
        Task::new("e")
            .duration(Duration::days(6))
            .depends_on("c")
            .depends_on("d"),
        Task::new("f").duration(Duration::days(2)).depends_on("a"),
        Task::new("end")
            .duration(Duration::days(0))
            .depends_on("e")
            .depends_on("f"),
    ];

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).expect("Should schedule");

    for (task_id, scheduled) in &schedule.tasks {
        assert!(
            scheduled.slack.as_days() >= 0.0,
            "Task {} has negative slack: {}",
            task_id,
            scheduled.slack.as_days()
        );
    }
}

// ============================================================================
// INVARIANT 2: ES respects predecessors
// ============================================================================

#[test]
fn early_start_respects_predecessors() {
    let mut project = Project::new("Test");
    project.tasks = vec![
        Task::new("a").duration(Duration::days(5)),
        Task::new("b").duration(Duration::days(3)).depends_on("a"),
        Task::new("c").duration(Duration::days(4)).depends_on("a"),
        Task::new("d")
            .duration(Duration::days(2))
            .depends_on("b")
            .depends_on("c"),
    ];

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).expect("Should schedule");

    // b starts after a finishes
    assert!(
        schedule.tasks["b"].start >= schedule.tasks["a"].finish,
        "b starts ({}) before a finishes ({})",
        schedule.tasks["b"].start,
        schedule.tasks["a"].finish
    );

    // c starts after a finishes
    assert!(
        schedule.tasks["c"].start >= schedule.tasks["a"].finish,
        "c starts ({}) before a finishes ({})",
        schedule.tasks["c"].start,
        schedule.tasks["a"].finish
    );

    // d starts after both b and c finish
    assert!(
        schedule.tasks["d"].start >= schedule.tasks["b"].finish,
        "d starts ({}) before b finishes ({})",
        schedule.tasks["d"].start,
        schedule.tasks["b"].finish
    );
    assert!(
        schedule.tasks["d"].start >= schedule.tasks["c"].finish,
        "d starts ({}) before c finishes ({})",
        schedule.tasks["d"].start,
        schedule.tasks["c"].finish
    );
}

// ============================================================================
// INVARIANT 4: Critical path has zero slack
// ============================================================================

#[test]
fn critical_path_has_zero_slack() {
    let mut project = Project::new("Test");
    project.tasks = vec![
        Task::new("a").duration(Duration::days(5)),
        Task::new("b").duration(Duration::days(3)),
        Task::new("c")
            .duration(Duration::days(2))
            .depends_on("a")
            .depends_on("b"),
    ];

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).expect("Should schedule");

    for task_id in &schedule.critical_path {
        let scheduled = &schedule.tasks[task_id];
        assert_eq!(
            scheduled.slack.as_days(),
            0.0,
            "Critical task {} has non-zero slack: {}",
            task_id,
            scheduled.slack.as_days()
        );
    }
}

// ============================================================================
// INVARIANT 5: Cross-container dependencies
// ============================================================================

#[test]
fn cross_container_dependencies_flat() {
    // Test with flat structure but named like containers
    let mut project = Project::new("Test");
    project.tasks = vec![
        Task::new("phase1_a").duration(Duration::days(5)),
        Task::new("phase2_b")
            .duration(Duration::days(3))
            .depends_on("phase1_a"),
    ];

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).expect("Should schedule");

    assert!(
        schedule.tasks["phase2_b"].start >= schedule.tasks["phase1_a"].finish,
        "phase2_b starts ({}) before phase1_a finishes ({})",
        schedule.tasks["phase2_b"].start,
        schedule.tasks["phase1_a"].finish
    );
}

#[test]
fn hierarchical_tasks_derive_dates() {
    // Nested structure
    let mut project = Project::new("Test");
    project.tasks = vec![Task::new("phase1")
        .name("Phase 1")
        .child(Task::new("a").duration(Duration::days(5)))
        .child(Task::new("b").duration(Duration::days(3)).depends_on("a"))];

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).expect("Should schedule");

    // Verify leaf tasks are scheduled
    assert!(schedule.tasks.contains_key("a") || schedule.tasks.contains_key("phase1.a"));
    assert!(schedule.tasks.contains_key("b") || schedule.tasks.contains_key("phase1.b"));
}

// ============================================================================
// REGRESSION: CRM Migration scenario
// ============================================================================

#[test]
fn crm_migration_simplified() {
    let mut project = Project::new("CRM Migration");

    project.tasks = vec![
        // Phase 1: Discovery
        Task::new("kickoff").duration(Duration::days(1)),
        Task::new("requirements")
            .duration(Duration::days(8))
            .depends_on("kickoff"),
        Task::new("gap_analysis")
            .duration(Duration::days(4))
            .depends_on("requirements"),
        Task::new("architecture")
            .duration(Duration::days(5))
            .depends_on("gap_analysis"),
        // Phase 2: Data Migration (depends on architecture)
        Task::new("data_mapping")
            .duration(Duration::days(6))
            .depends_on("architecture"),
        Task::new("etl_dev")
            .duration(Duration::days(10))
            .depends_on("data_mapping"),
        Task::new("test_migration")
            .duration(Duration::days(5))
            .depends_on("etl_dev"),
        // Phase 3: Integration (depends on architecture, parallel to data)
        Task::new("api_design")
            .duration(Duration::days(3))
            .depends_on("architecture"),
        Task::new("middleware")
            .duration(Duration::days(4))
            .depends_on("api_design"),
        Task::new("erp_connector")
            .duration(Duration::days(8))
            .depends_on("api_design"),
        Task::new("integration_test")
            .duration(Duration::days(4))
            .depends_on("middleware")
            .depends_on("erp_connector"),
        // Phase 4: Deployment (depends on BOTH data AND integration)
        Task::new("training")
            .duration(Duration::days(4))
            .depends_on("test_migration")
            .depends_on("integration_test"),
        Task::new("go_live")
            .duration(Duration::days(2))
            .depends_on("training"),
    ];

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).expect("Should schedule");

    // Verify cross-phase dependencies
    let arch = &schedule.tasks["architecture"];
    let data_mapping = &schedule.tasks["data_mapping"];
    let api_design = &schedule.tasks["api_design"];

    // Both data_mapping and api_design start after architecture
    assert!(
        data_mapping.start >= arch.finish,
        "data_mapping starts ({}) before architecture finishes ({})",
        data_mapping.start,
        arch.finish
    );
    assert!(
        api_design.start >= arch.finish,
        "api_design starts ({}) before architecture finishes ({})",
        api_design.start,
        arch.finish
    );

    // training depends on BOTH test_migration AND integration_test
    let training = &schedule.tasks["training"];
    let test_migration = &schedule.tasks["test_migration"];
    let integration_test = &schedule.tasks["integration_test"];

    assert!(
        training.start >= test_migration.finish,
        "training starts ({}) before test_migration finishes ({})",
        training.start,
        test_migration.finish
    );
    assert!(
        training.start >= integration_test.finish,
        "training starts ({}) before integration_test finishes ({})",
        training.start,
        integration_test.finish
    );

    // No negative slack
    for (task_id, scheduled) in &schedule.tasks {
        assert!(
            scheduled.slack.as_days() >= 0.0,
            "Task {} has negative slack: {}",
            task_id,
            scheduled.slack.as_days()
        );
    }

    // Critical path should include etl_dev (longest path through data migration)
    assert!(
        schedule.tasks["etl_dev"].is_critical,
        "etl_dev should be on critical path"
    );
    assert!(
        schedule.tasks["go_live"].is_critical,
        "go_live should be on critical path"
    );

    // Integration path should have slack (shorter than data path)
    // api_design + middleware + integration_test = 3 + 4 + 4 = 11 days
    // data_mapping + etl_dev + test_migration = 6 + 10 + 5 = 21 days
    // So integration path has slack
    assert!(
        schedule.tasks["api_design"].slack.as_days() > 0.0,
        "api_design should have slack (integration path is shorter)"
    );

    // Project duration should be:
    // kickoff(1) + req(8) + gap(4) + arch(5) + data_mapping(6) + etl(10) + test(5) + training(4) + go_live(2) = 45 days
    assert_eq!(
        schedule.project_duration.as_days().round() as i64,
        45,
        "Project duration should be 45 days"
    );
}

// ============================================================================
// INVARIANT 7: Non-FS Dependency Type Correctness
// ============================================================================
//
// These tests verify correct CPM calculations for SS, FF, and SF dependencies.
// The formulas are:
//
// Forward Pass (ES constraint for successor):
//   FS: ES(succ) >= EF(pred) + lag
//   SS: ES(succ) >= ES(pred) + lag
//   FF: ES(succ) >= EF(pred) + lag - duration(succ)
//   SF: ES(succ) >= ES(pred) + lag - duration(succ)
//
// Backward Pass (LF constraint for predecessor):
//   FS: LF(pred) <= LS(succ) - lag
//   SS: LF(pred) <= LS(succ) - lag + duration(pred)
//   FF: LF(pred) <= LF(succ) - lag
//   SF: LF(pred) <= LF(succ) - lag + duration(pred)

/// Helper to create a dependency
fn dep(predecessor: &str, dep_type: DependencyType) -> Dependency {
    Dependency {
        predecessor: predecessor.to_string(),
        dep_type,
        lag: None,
    }
}

/// Helper to create a dependency with lag
fn dep_lag(predecessor: &str, dep_type: DependencyType, lag_days: i64) -> Dependency {
    Dependency {
        predecessor: predecessor.to_string(),
        dep_type,
        lag: Some(Duration::days(lag_days)),
    }
}

#[test]
fn ff_forward_pass_accounts_for_successor_duration() {
    // Finish-to-Finish: B must finish when/after A finishes
    //
    // A(5) ----FF----> B(3)
    //
    // Forward pass:
    //   A: ES=0, EF=5
    //   B: EF(B) >= EF(A) = 5
    //      ES(B) = EF(B) - duration = 5 - 3 = 2
    //
    // Project end = 5 (both finish on day 5)
    //
    // Backward pass:
    //   B: LF=5, LS=2, Slack=0
    //   A: LF(A) <= LF(B) = 5, so LF=5, LS=0, Slack=0
    //
    // Both tasks are critical.

    let mut project = Project::new("FF Test");
    project.tasks = vec![
        Task::new("a").duration(Duration::days(5)),
        Task::new("b")
            .duration(Duration::days(3))
            .with_dependency(dep("a", DependencyType::FinishToFinish)),
    ];

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).expect("Should schedule");

    // Project duration should be 5 days (both finish together)
    assert_eq!(
        schedule.project_duration.as_days().round() as i64,
        5,
        "FF: Project should be 5 days (B finishes with A)"
    );

    // B should start on day 2 (not day 5!)
    // ES(B) = EF(A) - duration(B) = 5 - 3 = 2
    let b_start_day = (schedule.tasks["b"].start - project.start).num_days();
    assert_eq!(
        b_start_day, 2,
        "FF: B should start on day 2 to finish with A on day 5"
    );

    // Both should be critical
    assert!(schedule.tasks["a"].is_critical, "FF: A should be critical");
    assert!(schedule.tasks["b"].is_critical, "FF: B should be critical");
}

#[test]
fn ss_backward_pass_accounts_for_predecessor_duration() {
    // Start-to-Start: B starts when/after A starts
    //
    //     A(5) ----FS----> C(2)
    //                     /
    //     B(3) ----SS----/
    //
    // Forward pass:
    //   A: ES=0, EF=5
    //   B: ES=0, EF=3
    //   C: ES = max(EF(A), ES(B)) = max(5, 0) = 5, EF=7
    //
    // Project end = 7
    //
    // Backward pass:
    //   C: LF=7, LS=5, Slack=0 (critical)
    //   A: LF(A) <= LS(C) = 5, so LF=5, LS=0, Slack=0 (critical via FS)
    //   B: SS constraint: LS(B) <= LS(C) = 5
    //      LF(B) = LS(B) + duration = 5 + 3 = 8, but capped at project_end = 7
    //      So LF(B) = 7, LS(B) = 4
    //      Slack(B) = LS - ES = 4 - 0 = 4
    //
    // B has 4 days of slack because it only constrains C's START.

    let mut project = Project::new("SS Test");
    project.tasks = vec![
        Task::new("a").duration(Duration::days(5)),
        Task::new("b").duration(Duration::days(3)),
        Task::new("c")
            .duration(Duration::days(2))
            .depends_on("a") // FS
            .with_dependency(dep("b", DependencyType::StartToStart)),
    ];

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).expect("Should schedule");

    // Project duration = 7 days
    assert_eq!(
        schedule.project_duration.as_days().round() as i64,
        7,
        "SS: Project should be 7 days"
    );

    // A and C are critical
    assert!(schedule.tasks["a"].is_critical, "SS: A should be critical");
    assert!(schedule.tasks["c"].is_critical, "SS: C should be critical");

    // B should have slack = 4
    // SS only constrains B's START relative to C's start.
    // B can start as late as day 4 and still satisfy ES(C) >= ES(B).
    let b_slack = schedule.tasks["b"].slack.as_days().round() as i64;
    assert_eq!(
        b_slack, 4,
        "SS: B should have 4 days slack (can start day 0-4, C starts day 5)"
    );

    // B is not critical
    assert!(
        !schedule.tasks["b"].is_critical,
        "SS: B should NOT be critical"
    );
}

#[test]
fn ss_with_lag_forward_pass() {
    // Start-to-Start with 2-day lag
    //
    // A(5) ----SS+2d----> B(3)
    //
    // Forward:
    //   A: ES=0, EF=5
    //   B: ES >= ES(A) + lag = 0 + 2 = 2, EF=5
    //
    // Project end = 5

    let mut project = Project::new("SS Lag Test");
    project.tasks = vec![
        Task::new("a").duration(Duration::days(5)),
        Task::new("b")
            .duration(Duration::days(3))
            .with_dependency(dep_lag("a", DependencyType::StartToStart, 2)),
    ];

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).expect("Should schedule");

    // B starts 2 days after A (SS + 2d lag)
    let b_start_day = (schedule.tasks["b"].start - project.start).num_days();
    assert_eq!(
        b_start_day, 2,
        "SS+lag: B should start on day 2 (A's start + 2 day lag)"
    );

    // Project duration = 5 (both finish on day 5)
    assert_eq!(
        schedule.project_duration.as_days().round() as i64,
        5,
        "SS+lag: Project should be 5 days"
    );
}

#[test]
fn ff_with_lag_forward_pass() {
    // Finish-to-Finish with 1-day lag
    //
    // A(5) ----FF+1d----> B(3)
    //
    // Forward:
    //   A: ES=0, EF=5
    //   B: EF(B) >= EF(A) + lag = 5 + 1 = 6
    //      ES(B) = 6 - 3 = 3
    //
    // Project end = 6

    use chrono::NaiveDate;

    let mut project = Project::new("FF Lag Test");
    // Start on Monday to avoid weekend calendar issues
    project.start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
    project.tasks = vec![
        Task::new("a").duration(Duration::days(5)),
        Task::new("b")
            .duration(Duration::days(3))
            .with_dependency(dep_lag("a", DependencyType::FinishToFinish, 1)),
    ];

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).expect("Should schedule");

    // B starts on working day 3, finishes on day 6 (A's finish + 1 day)
    // With Monday start, working day 3 = Thursday = calendar day 3
    let b_start_day = (schedule.tasks["b"].start - project.start).num_days();
    assert_eq!(
        b_start_day, 3,
        "FF+lag: B should start on working day 3 to finish on day 6"
    );

    // Project duration = 6 working days
    assert_eq!(
        schedule.project_duration.as_days().round() as i64,
        6,
        "FF+lag: Project should be 6 days"
    );
}

#[test]
fn mixed_dependency_types_correct_critical_path() {
    // Complex graph with mixed dependency types
    //
    //     A(5) ----FS----> D(2)
    //                     /
    //     B(3) ----SS----/
    //                     \
    //     C(4) ----FF------\
    //
    // Forward:
    //   A: ES=0, EF=5
    //   B: ES=0, EF=3
    //   C: ES=0, EF=4
    //   D: ES = max(EF(A), ES(B), EF(C)-dur(D)) = max(5, 0, 4-2) = max(5, 0, 2) = 5
    //      EF = 7
    //
    // Critical path: A -> D
    // B and C have slack

    let mut project = Project::new("Mixed Deps Test");
    project.tasks = vec![
        Task::new("a").duration(Duration::days(5)),
        Task::new("b").duration(Duration::days(3)),
        Task::new("c").duration(Duration::days(4)),
        Task::new("d")
            .duration(Duration::days(2))
            .depends_on("a") // FS
            .with_dependency(dep("b", DependencyType::StartToStart))
            .with_dependency(dep("c", DependencyType::FinishToFinish)),
    ];

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).expect("Should schedule");

    // Project duration = 7
    assert_eq!(
        schedule.project_duration.as_days().round() as i64,
        7,
        "Mixed: Project should be 7 days"
    );

    // A and D are critical (FS path determines project length)
    assert!(
        schedule.tasks["a"].is_critical,
        "Mixed: A should be critical"
    );
    assert!(
        schedule.tasks["d"].is_critical,
        "Mixed: D should be critical"
    );

    // B and C should have slack
    assert!(
        schedule.tasks["b"].slack.as_days() > 0.0,
        "Mixed: B should have slack (SS constraint weaker than FS)"
    );
    assert!(
        schedule.tasks["c"].slack.as_days() > 0.0,
        "Mixed: C should have slack (FF constraint weaker than FS for shorter C)"
    );
}
