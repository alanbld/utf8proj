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
fn empty_container_treated_as_milestone() {
    // Given: Container with no children and no duration
    // When: Schedule is computed
    // Then: Container is treated as a zero-duration task (milestone)

    let mut project = Project::new("Empty Container Test");
    project.start = date(2025, 2, 3);

    // Empty container - no children, no duration
    project.tasks = vec![Task::new("phase1")];

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).unwrap();

    let phase1 = &schedule.tasks["phase1"];

    // Empty container should be zero duration
    assert_eq!(phase1.duration, Duration::zero(), "Empty container should have zero duration");

    // Should start at project start
    assert_eq!(phase1.start, date(2025, 2, 3), "Empty container should start at project start");

    // Start and finish should be the same (like a milestone)
    assert_eq!(phase1.start, phase1.finish, "Empty container start/finish should match");
}

#[test]
fn empty_container_with_dependencies() {
    // Given: Empty container that depends on another task
    // When: Schedule is computed
    // Then: Container starts after the dependency

    let mut project = Project::new("Empty Container Dependency Test");
    project.start = date(2025, 2, 3);

    project.tasks = vec![
        Task::new("setup").effort(Duration::days(5)),
        Task::new("phase1").depends_on("setup"), // Empty container depending on setup
    ];

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).unwrap();

    let setup = &schedule.tasks["setup"];
    let phase1 = &schedule.tasks["phase1"];

    // Empty container should start after setup finishes
    assert!(
        phase1.start > setup.finish,
        "Empty container should start after dependency: phase1.start={}, setup.finish={}",
        phase1.start,
        setup.finish
    );
}

#[test]
fn task_depends_on_empty_container() {
    // Given: Task that depends on an empty container
    // When: Schedule is computed
    // Then: Task starts after the empty container

    let mut project = Project::new("Depend on Empty Container Test");
    project.start = date(2025, 2, 3);

    project.tasks = vec![
        Task::new("phase1"), // Empty container
        Task::new("work").effort(Duration::days(5)).depends_on("phase1"),
    ];

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).unwrap();

    let phase1 = &schedule.tasks["phase1"];
    let work = &schedule.tasks["work"];

    // Empty container at project start
    assert_eq!(phase1.start, date(2025, 2, 3));

    // Work should start at or after phase1 (which is at project start)
    assert!(
        work.start >= phase1.start,
        "Work should start after empty container"
    );
}

#[test]
fn nested_empty_containers() {
    // Given: Nested containers where inner is empty
    // When: Schedule is computed
    // Then: Both containers treated appropriately

    let mut project = Project::new("Nested Empty Container Test");
    project.start = date(2025, 2, 3);

    // Outer container with one child that is also empty
    project.tasks = vec![Task::new("phase1").child(Task::new("subphase"))];

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).unwrap();

    // Inner empty container
    let subphase = &schedule.tasks["phase1.subphase"];
    assert_eq!(subphase.duration, Duration::zero());

    // Outer container derives dates from inner (which is zero)
    let phase1 = &schedule.tasks["phase1"];
    assert_eq!(phase1.start, subphase.start);
    assert_eq!(phase1.finish, subphase.finish);
}

#[test]
fn deep_nesting_performance() {
    // Given: 5+ levels of nesting with 100+ tasks
    // When: Schedule is computed
    // Then: Completes in reasonable time (<1s)

    use std::time::Instant;

    let mut project = Project::new("Deep Nesting Performance Test");
    project.start = date(2025, 2, 3);

    // Create a deeply nested structure:
    // 5 top-level phases, each with 4 sub-phases, each with 5 tasks = 100+ tasks
    // Plus dependencies between tasks
    let mut top_tasks = Vec::new();

    for phase_num in 1..=5 {
        let phase_id = format!("phase{}", phase_num);
        let mut phase = Task::new(&phase_id);

        for sub_num in 1..=4 {
            let sub_id = format!("sub{}", sub_num);
            let mut sub_phase = Task::new(&sub_id);

            for task_num in 1..=5 {
                let task_id = format!("task{}", task_num);
                let mut task = Task::new(&task_id).effort(Duration::days(2));

                // Add dependency on previous task in same sub-phase
                if task_num > 1 {
                    task = task.depends_on(&format!("task{}", task_num - 1));
                }

                sub_phase = sub_phase.child(task);
            }

            // Add dependency on previous sub-phase's last task
            if sub_num > 1 {
                // The sub-phase itself depends on the previous sub-phase
                sub_phase = sub_phase.depends_on(&format!("sub{}.task5", sub_num - 1));
            }

            phase = phase.child(sub_phase);
        }

        // Add dependency on previous phase
        if phase_num > 1 {
            phase = phase.depends_on(&format!("phase{}", phase_num - 1));
        }

        top_tasks.push(phase);
    }

    project.tasks = top_tasks;

    // Count total tasks
    fn count_tasks(tasks: &[Task]) -> usize {
        tasks.iter().map(|t| 1 + count_tasks(&t.children)).sum()
    }
    let total_tasks = count_tasks(&project.tasks);
    assert!(total_tasks >= 100, "Should have 100+ tasks, got {}", total_tasks);

    // Measure scheduling time
    let start_time = Instant::now();
    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).unwrap();
    let elapsed = start_time.elapsed();

    // Should complete in under 1 second
    assert!(
        elapsed.as_secs_f64() < 1.0,
        "Scheduling {} tasks took {:.2}s, should be < 1s",
        total_tasks,
        elapsed.as_secs_f64()
    );

    // Verify schedule is valid
    assert_eq!(schedule.tasks.len(), total_tasks);

    // Print performance info (visible with --nocapture)
    println!(
        "Deep nesting performance: {} tasks scheduled in {:.3}ms",
        total_tasks,
        elapsed.as_secs_f64() * 1000.0
    );
}

#[test]
fn very_wide_hierarchy_performance() {
    // Given: Wide hierarchy (many siblings at each level)
    // When: Schedule is computed
    // Then: Completes in reasonable time

    use std::time::Instant;

    let mut project = Project::new("Wide Hierarchy Performance Test");
    project.start = date(2025, 2, 3);

    // Create wide structure: 10 phases with 20 tasks each = 200+ tasks
    let mut top_tasks = Vec::new();
    let mut prev_phase: Option<String> = None;

    for phase_num in 1..=10 {
        let phase_id = format!("phase{}", phase_num);
        let mut phase = Task::new(&phase_id);

        // Add dependency on previous phase
        if let Some(ref prev) = prev_phase {
            phase = phase.depends_on(prev);
        }

        for task_num in 1..=20 {
            let task_id = format!("task{}", task_num);
            let task = Task::new(&task_id).effort(Duration::days(1));
            phase = phase.child(task);
        }

        prev_phase = Some(phase_id.clone());
        top_tasks.push(phase);
    }

    project.tasks = top_tasks;

    fn count_tasks(tasks: &[Task]) -> usize {
        tasks.iter().map(|t| 1 + count_tasks(&t.children)).sum()
    }
    let total_tasks = count_tasks(&project.tasks);

    let start_time = Instant::now();
    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).unwrap();
    let elapsed = start_time.elapsed();

    assert!(
        elapsed.as_secs_f64() < 1.0,
        "Scheduling {} tasks took {:.2}s, should be < 1s",
        total_tasks,
        elapsed.as_secs_f64()
    );

    assert_eq!(schedule.tasks.len(), total_tasks);

    println!(
        "Wide hierarchy performance: {} tasks scheduled in {:.3}ms",
        total_tasks,
        elapsed.as_secs_f64() * 1000.0
    );
}

#[test]
fn complex_dependency_graph_performance() {
    // Given: Complex dependency graph with cross-container dependencies
    // When: Schedule is computed
    // Then: Completes in reasonable time

    use std::time::Instant;

    let mut project = Project::new("Complex Dependency Performance Test");
    project.start = date(2025, 2, 3);

    // Create tasks with complex cross-dependencies
    let mut tasks = Vec::new();

    // Create 50 flat tasks with varying dependencies
    for i in 1..=50 {
        let mut task = Task::new(&format!("task{}", i)).effort(Duration::days(1));

        // Each task depends on up to 3 previous tasks
        if i > 1 {
            task = task.depends_on(&format!("task{}", i - 1));
        }
        if i > 5 {
            task = task.depends_on(&format!("task{}", i - 5));
        }
        if i > 10 {
            task = task.depends_on(&format!("task{}", i - 10));
        }

        tasks.push(task);
    }

    project.tasks = tasks;

    let start_time = Instant::now();
    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).unwrap();
    let elapsed = start_time.elapsed();

    assert!(
        elapsed.as_secs_f64() < 1.0,
        "Scheduling 50 tasks with complex deps took {:.2}s, should be < 1s",
        elapsed.as_secs_f64()
    );

    assert_eq!(schedule.tasks.len(), 50);

    println!(
        "Complex dependency performance: 50 tasks scheduled in {:.3}ms",
        elapsed.as_secs_f64() * 1000.0
    );
}
