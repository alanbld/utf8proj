//! TDD Test Suite: Hierarchical Task Scheduling
//!
//! These tests define the expected scheduling behavior for nested tasks
//! and advanced dependency types. Run with: cargo test --test hierarchical_scheduling
//!
//! Test progression matches SPEC_HIERARCHICAL_TASKS.md

use chrono::NaiveDate;
use utf8proj_core::{Duration, Project, Scheduler, Task, TaskConstraint};
use utf8proj_solver::CpmSolver;

fn date(year: i32, month: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(year, month, day).unwrap()
}

// =============================================================================
// Phase 2: Flat Task Scheduling (Current Capability)
// =============================================================================

#[test]
fn schedule_flat_tasks_with_constraints() {
    // This should already work - validates current capability
    // ttg_01_base.tjp equivalent
}

#[test]
fn schedule_flat_tasks_with_fs_dependencies() {
    // This should already work - validates current capability
    // ttg_02_deps.tjp equivalent
}

// =============================================================================
// Phase 3: Container Task Date Derivation
// =============================================================================

#[test]
fn container_start_derived_from_earliest_child() {
    // Given: Container with children starting at different dates
    // When: Schedule is computed
    // Then: Container.start = min(child.start)

    let mut project = Project::new("Container Start Test");
    project.start = date(2025, 2, 3); // Monday

    // task phase1 {
    //     task act1 { start 2025-02-10 length 5d }
    //     task act2 { start 2025-02-03 length 5d }  // earlier
    // }
    let mut act1 = Task::new("act1").effort(Duration::days(5));
    act1.constraints.push(TaskConstraint::MustStartOn(date(2025, 2, 10)));

    let mut act2 = Task::new("act2").effort(Duration::days(5));
    act2.constraints.push(TaskConstraint::MustStartOn(date(2025, 2, 3)));

    project.tasks = vec![Task::new("phase1").child(act1).child(act2)];

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).unwrap();

    // Container start should be the earliest child start (2025-02-03)
    let phase1 = &schedule.tasks["phase1"];
    assert_eq!(phase1.start, date(2025, 2, 3), "Container start should be min of children");
}

#[test]
fn container_finish_derived_from_latest_child() {
    // Given: Container with children finishing at different dates
    // When: Schedule is computed
    // Then: Container.finish = max(child.finish)

    let mut project = Project::new("Container Finish Test");
    project.start = date(2025, 2, 3); // Monday

    // task phase1 {
    //     task act1 { length 10d }  // finishes later
    //     task act2 { length 5d }
    // }
    project.tasks = vec![Task::new("phase1")
        .child(Task::new("act1").effort(Duration::days(10)))
        .child(Task::new("act2").effort(Duration::days(5)))];

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).unwrap();

    // act1: 10 days from Feb 03 -> finishes Feb 14 (last day of work)
    // act2: 5 days from Feb 03 -> finishes Feb 07
    // Container finish should be Feb 14
    let phase1 = &schedule.tasks["phase1"];
    let act1 = &schedule.tasks["phase1.act1"];

    assert_eq!(
        phase1.finish, act1.finish,
        "Container finish should be max of children"
    );
}

#[test]
fn container_dependency_waits_for_all_children() {
    // Given: Task depending on container
    // When: Schedule is computed
    // Then: Dependent starts after ALL container children finish

    let mut project = Project::new("Container Dependency Test");
    project.start = date(2025, 2, 3); // Monday

    // task phase1 {
    //     task act1 { length 10d }
    //     task act2 { length 20d }  // finishes later
    // }
    // task phase2 {
    //     depends phase1  // depends on container
    //     length 5d
    // }
    project.tasks = vec![
        Task::new("phase1")
            .child(Task::new("act1").effort(Duration::days(10)))
            .child(Task::new("act2").effort(Duration::days(20))),
        Task::new("phase2")
            .effort(Duration::days(5))
            .depends_on("phase1"),
    ];

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).unwrap();

    // phase1.act2 finishes after 20 days = Feb 28
    // phase2 should start after phase1 finishes (after Feb 28)
    let phase1 = &schedule.tasks["phase1"];
    let phase2 = &schedule.tasks["phase2"];

    assert!(
        phase2.start > phase1.finish,
        "phase2 should start after phase1 finishes: phase2.start={}, phase1.finish={}",
        phase2.start,
        phase1.finish
    );
}

// =============================================================================
// Phase 4: SS/FF/SF Dependency Scheduling
// =============================================================================

#[test]
fn ss_dependency_aligns_starts() {
    // Given: Task B depends on Task A with SS relationship
    // When: Schedule is computed
    // Then: B.start >= A.start

    let mut project = Project::new("SS Dependency Test");
    project.start = date(2025, 2, 3); // Monday

    // act1 with constrained start, act2 depends SS on act1
    let mut act1 = Task::new("act1").effort(Duration::days(20));
    act1.constraints.push(TaskConstraint::MustStartOn(date(2025, 2, 3)));

    let mut act2 = Task::new("act2").effort(Duration::days(10));
    act2.depends.push(utf8proj_core::Dependency {
        predecessor: "act1".to_string(),
        dep_type: utf8proj_core::DependencyType::StartToStart,
        lag: None,
    });

    project.tasks = vec![act1, act2];

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).unwrap();

    // act2.start should equal act1.start
    assert_eq!(
        schedule.tasks["act2"].start,
        schedule.tasks["act1"].start,
        "SS: act2 should start when act1 starts"
    );
}

#[test]
fn ss_dependency_with_positive_lag() {
    // Given: Task B depends on Task A with SS+5d
    // When: Schedule is computed
    // Then: B.start >= A.start + 5d

    let mut project = Project::new("SS+Lag Dependency Test");
    project.start = date(2025, 2, 3); // Monday

    let mut act1 = Task::new("act1").effort(Duration::days(20));
    act1.constraints.push(TaskConstraint::MustStartOn(date(2025, 2, 3)));

    let mut act2 = Task::new("act2").effort(Duration::days(10));
    act2.depends.push(utf8proj_core::Dependency {
        predecessor: "act1".to_string(),
        dep_type: utf8proj_core::DependencyType::StartToStart,
        lag: Some(Duration::days(5)),
    });

    project.tasks = vec![act1, act2];

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).unwrap();

    // act2.start should be act1.start + 5 working days
    // Feb 03 + 5 working days = Feb 10
    assert_eq!(
        schedule.tasks["act2"].start,
        date(2025, 2, 10),
        "SS+5d: act2 should start 5 days after act1 starts"
    );
}

#[test]
fn ff_dependency_aligns_finishes() {
    // Given: Task B depends on Task A with FF relationship
    // When: Schedule is computed
    // Then: B.finish >= A.finish

    let mut project = Project::new("FF Dependency Test");
    project.start = date(2025, 2, 3); // Monday

    let mut act1 = Task::new("act1").effort(Duration::days(20));
    act1.constraints.push(TaskConstraint::MustStartOn(date(2025, 2, 3)));

    let mut act2 = Task::new("act2").effort(Duration::days(10));
    act2.depends.push(utf8proj_core::Dependency {
        predecessor: "act1".to_string(),
        dep_type: utf8proj_core::DependencyType::FinishToFinish,
        lag: None,
    });

    project.tasks = vec![act1, act2];

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).unwrap();

    // act1 finishes after 20 days = Feb 28
    // act2 with FF should finish on same day as act1
    assert_eq!(
        schedule.tasks["act2"].finish,
        schedule.tasks["act1"].finish,
        "FF: act2 should finish when act1 finishes"
    );
}

#[test]
fn sf_dependency_finish_after_start() {
    // Given: Task B depends on Task A with SF relationship
    // When: Schedule is computed
    // Then: B.finish >= A.start

    let mut project = Project::new("SF Dependency Test");
    project.start = date(2025, 2, 3); // Monday

    // act1 starts Feb 10
    let mut act1 = Task::new("act1").effort(Duration::days(20));
    act1.constraints.push(TaskConstraint::MustStartOn(date(2025, 2, 10)));

    // act2 with SF depends on act1 - act2.finish >= act1.start
    let mut act2 = Task::new("act2").effort(Duration::days(10));
    act2.depends.push(utf8proj_core::Dependency {
        predecessor: "act1".to_string(),
        dep_type: utf8proj_core::DependencyType::StartToFinish,
        lag: None,
    });

    project.tasks = vec![act1, act2];

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).unwrap();

    // act2.finish >= act1.start (Feb 10)
    // With 10d duration, act2 could start as early as Jan 27
    // But project starts Feb 03, so act2.start = Feb 03, finish = Feb 14
    // However, SF constraint says finish >= Feb 10, which is satisfied
    assert!(
        schedule.tasks["act2"].finish >= schedule.tasks["act1"].start,
        "SF: act2 finish ({}) should be >= act1 start ({})",
        schedule.tasks["act2"].finish,
        schedule.tasks["act1"].start
    );
}

// =============================================================================
// Phase 5: Negative Lag (Lead) Scheduling
// =============================================================================

#[test]
fn negative_lag_allows_overlap() {
    // Given: Task B depends on Task A with FS-5d (5 day lead)
    // When: Schedule is computed
    // Then: B.start >= A.finish - 5d (overlap allowed)

    let mut project = Project::new("Negative Lag Test");
    project.start = date(2025, 2, 3); // Monday

    // act1: 20 days starting Feb 03, finishes Feb 28
    let mut act1 = Task::new("act1").effort(Duration::days(20));
    act1.constraints.push(TaskConstraint::MustStartOn(date(2025, 2, 3)));

    // act2 depends on act1 with -5d lag (5 day lead)
    // B.start >= A.finish - 5d = Feb 28 - 5d = Feb 21
    let mut act2 = Task::new("act2").effort(Duration::days(10));
    act2.depends.push(utf8proj_core::Dependency {
        predecessor: "act1".to_string(),
        dep_type: utf8proj_core::DependencyType::FinishToStart,
        lag: Some(Duration::days(-5)),
    });

    project.tasks = vec![act1, act2];

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).unwrap();

    // act1 finishes Feb 28 (20 working days from Feb 03)
    // act2 should start Feb 21 (5 working days before Feb 28)
    // Feb 28 - 5 working days = Feb 21
    assert_eq!(
        schedule.tasks["act2"].start,
        date(2025, 2, 21),
        "FS-5d: act2 should start 5 days before act1 finishes"
    );

    // Verify overlap: act2 starts before act1 finishes
    assert!(
        schedule.tasks["act2"].start < schedule.tasks["act1"].finish,
        "FS-5d: act2 should overlap with act1"
    );
}

#[test]
fn ss_negative_lag_acts_as_lead() {
    // Given: Task B depends on Task A with SS-3d
    // When: Schedule is computed
    // Then: B.start >= A.start - 3d

    let mut project = Project::new("SS Negative Lag Test");
    project.start = date(2025, 2, 3); // Monday

    // act1 starts Feb 10
    let mut act1 = Task::new("act1").effort(Duration::days(20));
    act1.constraints.push(TaskConstraint::MustStartOn(date(2025, 2, 10)));

    // act2 depends on act1 with SS-3d
    // B.start >= A.start - 3d = Feb 10 - 3d = Feb 05
    let mut act2 = Task::new("act2").effort(Duration::days(10));
    act2.depends.push(utf8proj_core::Dependency {
        predecessor: "act1".to_string(),
        dep_type: utf8proj_core::DependencyType::StartToStart,
        lag: Some(Duration::days(-3)),
    });

    project.tasks = vec![act1, act2];

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).unwrap();

    // act1 starts Feb 10
    // SS-3d means act2.start >= act1.start - 3d = Feb 10 - 3d = Feb 05
    // But project starts Feb 03, so constraint is Feb 05 (if it's positive)
    // Actually: Feb 10 - 3 working days = Feb 05
    assert_eq!(
        schedule.tasks["act2"].start,
        date(2025, 2, 5),
        "SS-3d: act2 should start 3 days before act1 starts"
    );

    // Verify: act2 starts before act1
    assert!(
        schedule.tasks["act2"].start < schedule.tasks["act1"].start,
        "SS-3d: act2 should start before act1"
    );
}

// =============================================================================
// Integration: Critical Path with Mixed Dependencies
// =============================================================================

#[test]
#[ignore = "Full integration: Not yet implemented"]
fn critical_path_with_ss_dependencies() {
    // TTG-style project with overlapping activities
    // Critical path should consider SS relationships

    // This represents the actual TTG schedule:
    // 1.1: Week 1-4 (validation)
    // 2.1: Week 2-14 (development, SS+1w from 1.1)
    // 3.1: Week 3-17 (ABL migration, SS+2w from 1.1)
    // 3.2: Week 5-15 (Shell migration, SS+2w from 3.1)
    // 3.3: Week 11-16 (Perl migration)
    // 4.1: Week 15-17 (cutover, FS from all)
}

#[test]
#[ignore = "Full integration: Not yet implemented"]
fn schedule_matches_tj3_output() {
    // Parse ttg_02_deps.tjp
    // Schedule with utf8proj
    // Compare dates against TJ3 scheduled output
    // Tolerance: 1 working day
}

// =============================================================================
// Edge Cases
// =============================================================================

#[test]
fn circular_dependency_detection() {
    // Given: A depends on B, B depends on A
    // When: Schedule is attempted
    // Then: Error is returned indicating circular dependency

    let mut project = Project::new("Circular Dependency Test");
    project.start = date(2025, 2, 3);

    // Simple circular: act1 -> act2 -> act1
    project.tasks = vec![
        Task::new("act1")
            .effort(Duration::days(5))
            .depends_on("act2"),
        Task::new("act2")
            .effort(Duration::days(5))
            .depends_on("act1"),
    ];

    let solver = CpmSolver::new();
    let result = solver.schedule(&project);

    assert!(result.is_err(), "Should detect circular dependency");
    let err = result.unwrap_err();
    assert!(
        matches!(err, utf8proj_core::ScheduleError::CircularDependency(_)),
        "Error should be CircularDependency, got: {:?}",
        err
    );
}

#[test]
fn circular_dependency_in_nested_tasks() {
    // Given: Nested tasks with circular dependencies
    // When: Schedule is attempted
    // Then: Error is returned

    let mut project = Project::new("Nested Circular Test");
    project.start = date(2025, 2, 3);

    // Circular within a container: phase1.act1 -> phase1.act2 -> phase1.act1
    project.tasks = vec![Task::new("phase1")
        .child(Task::new("act1").effort(Duration::days(5)).depends_on("act2"))
        .child(Task::new("act2").effort(Duration::days(5)).depends_on("act1"))];

    let solver = CpmSolver::new();
    let result = solver.schedule(&project);

    assert!(result.is_err(), "Should detect circular dependency in nested tasks");
}

#[test]
fn circular_dependency_across_containers() {
    // Given: Circular dependency across container boundaries
    // When: Schedule is attempted
    // Then: Error is returned

    let mut project = Project::new("Cross-Container Circular Test");
    project.start = date(2025, 2, 3);

    // Cross-container circular: phase1.act1 -> phase2.act1 -> phase1.act1
    project.tasks = vec![
        Task::new("phase1").child(
            Task::new("act1")
                .effort(Duration::days(5))
                .depends_on("phase2.act1"),
        ),
        Task::new("phase2").child(
            Task::new("act1")
                .effort(Duration::days(5))
                .depends_on("phase1.act1"),
        ),
    ];

    let solver = CpmSolver::new();
    let result = solver.schedule(&project);

    assert!(result.is_err(), "Should detect circular dependency across containers");
}

#[test]
fn three_task_cycle_detection() {
    // Given: A -> B -> C -> A (3-task cycle)
    // When: Schedule is attempted
    // Then: Error is returned

    let mut project = Project::new("Three Task Cycle Test");
    project.start = date(2025, 2, 3);

    project.tasks = vec![
        Task::new("a").effort(Duration::days(2)).depends_on("c"),
        Task::new("b").effort(Duration::days(2)).depends_on("a"),
        Task::new("c").effort(Duration::days(2)).depends_on("b"),
    ];

    let solver = CpmSolver::new();
    let result = solver.schedule(&project);

    assert!(result.is_err(), "Should detect 3-task circular dependency");
}

#[test]
#[ignore = "Edge case: Not yet implemented"]
fn empty_container_handling() {
    // Given: Container with no children
    // When: Schedule is computed
    // Then: Warning or error (container has no meaningful dates)

    // task phase1 { }  // no children
}

#[test]
#[ignore = "Edge case: Not yet implemented"]
fn deep_nesting_performance() {
    // Given: 5+ levels of nesting with 100+ tasks
    // When: Schedule is computed
    // Then: Completes in reasonable time (<1s)
}
