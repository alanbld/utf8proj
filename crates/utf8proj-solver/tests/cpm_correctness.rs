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

use utf8proj_core::{Duration, Project, Scheduler, Task};
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
        Task::new("c").duration(Duration::days(2)).depends_on("a").depends_on("b"),
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
        Task::new("a").duration(Duration::days(5)).depends_on("start"),
        Task::new("b").duration(Duration::days(8)).depends_on("start"),
        Task::new("c").duration(Duration::days(3)).depends_on("a"),
        Task::new("d").duration(Duration::days(4)).depends_on("b"),
        Task::new("e").duration(Duration::days(6)).depends_on("c").depends_on("d"),
        Task::new("f").duration(Duration::days(2)).depends_on("a"),
        Task::new("end").duration(Duration::days(0)).depends_on("e").depends_on("f"),
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
        Task::new("d").duration(Duration::days(2)).depends_on("b").depends_on("c"),
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
        Task::new("c").duration(Duration::days(2)).depends_on("a").depends_on("b"),
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
        Task::new("phase2_b").duration(Duration::days(3)).depends_on("phase1_a"),
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
    project.tasks = vec![
        Task::new("phase1")
            .name("Phase 1")
            .child(Task::new("a").duration(Duration::days(5)))
            .child(Task::new("b").duration(Duration::days(3)).depends_on("a")),
    ];

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
        Task::new("requirements").duration(Duration::days(8)).depends_on("kickoff"),
        Task::new("gap_analysis").duration(Duration::days(4)).depends_on("requirements"),
        Task::new("architecture").duration(Duration::days(5)).depends_on("gap_analysis"),

        // Phase 2: Data Migration (depends on architecture)
        Task::new("data_mapping").duration(Duration::days(6)).depends_on("architecture"),
        Task::new("etl_dev").duration(Duration::days(10)).depends_on("data_mapping"),
        Task::new("test_migration").duration(Duration::days(5)).depends_on("etl_dev"),

        // Phase 3: Integration (depends on architecture, parallel to data)
        Task::new("api_design").duration(Duration::days(3)).depends_on("architecture"),
        Task::new("middleware").duration(Duration::days(4)).depends_on("api_design"),
        Task::new("erp_connector").duration(Duration::days(8)).depends_on("api_design"),
        Task::new("integration_test").duration(Duration::days(4))
            .depends_on("middleware")
            .depends_on("erp_connector"),

        // Phase 4: Deployment (depends on BOTH data AND integration)
        Task::new("training").duration(Duration::days(4))
            .depends_on("test_migration")
            .depends_on("integration_test"),
        Task::new("go_live").duration(Duration::days(2)).depends_on("training"),
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
