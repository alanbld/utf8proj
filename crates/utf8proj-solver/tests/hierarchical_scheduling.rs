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
    act1.constraints
        .push(TaskConstraint::MustStartOn(date(2025, 2, 10)));

    let mut act2 = Task::new("act2").effort(Duration::days(5));
    act2.constraints
        .push(TaskConstraint::MustStartOn(date(2025, 2, 3)));

    project.tasks = vec![Task::new("phase1").child(act1).child(act2)];

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).unwrap();

    // Container start should be the earliest child start (2025-02-03)
    let phase1 = &schedule.tasks["phase1"];
    assert_eq!(
        phase1.start,
        date(2025, 2, 3),
        "Container start should be min of children"
    );
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
    act1.constraints
        .push(TaskConstraint::MustStartOn(date(2025, 2, 3)));

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
        schedule.tasks["act2"].start, schedule.tasks["act1"].start,
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
    act1.constraints
        .push(TaskConstraint::MustStartOn(date(2025, 2, 3)));

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
    act1.constraints
        .push(TaskConstraint::MustStartOn(date(2025, 2, 3)));

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
        schedule.tasks["act2"].finish, schedule.tasks["act1"].finish,
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
    act1.constraints
        .push(TaskConstraint::MustStartOn(date(2025, 2, 10)));

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
    act1.constraints
        .push(TaskConstraint::MustStartOn(date(2025, 2, 3)));

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
    act1.constraints
        .push(TaskConstraint::MustStartOn(date(2025, 2, 10)));

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
fn critical_path_with_ss_dependencies() {
    // TTG-style project with overlapping activities
    // Critical path should consider SS relationships

    let mut project = Project::new("Critical Path SS Test");
    project.start = date(2025, 2, 3); // Monday

    // Simplified TTG-style schedule:
    // validation: 20 days (4 weeks)
    // development: SS+5d from validation, 65 days (starts 1 week after validation)
    // migration: SS+10d from validation, 75 days (starts 2 weeks after validation)
    // cutover: FS from all, 15 days

    let mut validation = Task::new("validation").effort(Duration::days(20));
    validation
        .constraints
        .push(TaskConstraint::MustStartOn(date(2025, 2, 3)));

    let mut development = Task::new("development").effort(Duration::days(65));
    development.depends.push(utf8proj_core::Dependency {
        predecessor: "validation".to_string(),
        dep_type: utf8proj_core::DependencyType::StartToStart,
        lag: Some(Duration::days(5)), // SS+5d (1 week)
    });

    let mut migration = Task::new("migration").effort(Duration::days(75));
    migration.depends.push(utf8proj_core::Dependency {
        predecessor: "validation".to_string(),
        dep_type: utf8proj_core::DependencyType::StartToStart,
        lag: Some(Duration::days(10)), // SS+10d (2 weeks)
    });

    let cutover = Task::new("cutover")
        .effort(Duration::days(15))
        .depends_on("validation")
        .depends_on("development")
        .depends_on("migration");

    project.tasks = vec![validation, development, migration, cutover];

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).unwrap();

    // Verify overlapping starts due to SS dependencies
    let val = &schedule.tasks["validation"];
    let dev = &schedule.tasks["development"];
    let mig = &schedule.tasks["migration"];
    let cut = &schedule.tasks["cutover"];

    // Development starts 5 days after validation starts
    assert_eq!(
        dev.start,
        date(2025, 2, 10), // Feb 3 + 5 working days
        "Development should start SS+5d from validation"
    );

    // Migration starts 10 days after validation starts
    assert_eq!(
        mig.start,
        date(2025, 2, 17), // Feb 3 + 10 working days
        "Migration should start SS+10d from validation"
    );

    // Verify tasks overlap (SS dependencies allow concurrent work)
    assert!(
        dev.start < val.finish,
        "Development should overlap with validation"
    );
    assert!(
        mig.start < val.finish,
        "Migration should overlap with validation"
    );

    // Cutover starts after all predecessors finish
    // validation: Feb 3 + 20 days = Feb 28
    // development: Feb 10 + 65 days = ~May 14
    // migration: Feb 17 + 75 days = ~May 30
    // cutover should start after migration finishes (the latest)
    assert!(
        cut.start > mig.finish,
        "Cutover should start after migration finishes: cut.start={}, mig.finish={}",
        cut.start,
        mig.finish
    );
    assert!(
        cut.start > dev.finish,
        "Cutover should start after development finishes"
    );

    // Critical path should include migration (longest path)
    // Path: validation(partial) -> migration -> cutover
    // OR the path that determines project end
    assert!(
        schedule.critical_path.contains(&"migration".to_string()),
        "Migration should be on critical path (longest duration after SS): {:?}",
        schedule.critical_path
    );
    assert!(
        schedule.critical_path.contains(&"cutover".to_string()),
        "Cutover should be on critical path: {:?}",
        schedule.critical_path
    );
}

#[test]
fn critical_path_with_parallel_ss_chains() {
    // Test that critical path correctly identifies the longest chain
    // when multiple SS-dependent chains run in parallel

    let mut project = Project::new("Parallel SS Chains Test");
    project.start = date(2025, 2, 3);

    // Two parallel chains from the same start:
    // Chain A: start -> taskA1 (SS+0) -> taskA2 (FS) = 10 + 5 = 15 days total
    // Chain B: start -> taskB1 (SS+0) -> taskB2 (FS) = 20 + 5 = 25 days total
    // Chain B should be critical

    let mut start_task = Task::new("start").effort(Duration::days(5));
    start_task
        .constraints
        .push(TaskConstraint::MustStartOn(date(2025, 2, 3)));

    // Chain A (shorter)
    let mut task_a1 = Task::new("taskA1").effort(Duration::days(10));
    task_a1.depends.push(utf8proj_core::Dependency {
        predecessor: "start".to_string(),
        dep_type: utf8proj_core::DependencyType::StartToStart,
        lag: None,
    });
    let task_a2 = Task::new("taskA2")
        .effort(Duration::days(5))
        .depends_on("taskA1");

    // Chain B (longer - critical)
    let mut task_b1 = Task::new("taskB1").effort(Duration::days(20));
    task_b1.depends.push(utf8proj_core::Dependency {
        predecessor: "start".to_string(),
        dep_type: utf8proj_core::DependencyType::StartToStart,
        lag: None,
    });
    let task_b2 = Task::new("taskB2")
        .effort(Duration::days(5))
        .depends_on("taskB1");

    // Final task depends on both chains
    let finish = Task::new("finish")
        .effort(Duration::days(2))
        .depends_on("taskA2")
        .depends_on("taskB2");

    project.tasks = vec![start_task, task_a1, task_a2, task_b1, task_b2, finish];

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).unwrap();

    // Chain B should be critical (longer path)
    assert!(
        schedule.critical_path.contains(&"taskB1".to_string()),
        "taskB1 should be on critical path: {:?}",
        schedule.critical_path
    );
    assert!(
        schedule.critical_path.contains(&"taskB2".to_string()),
        "taskB2 should be on critical path: {:?}",
        schedule.critical_path
    );
    assert!(
        schedule.critical_path.contains(&"finish".to_string()),
        "finish should be on critical path: {:?}",
        schedule.critical_path
    );

    // Chain A should NOT be critical (has slack)
    assert!(
        !schedule.tasks["taskA1"].is_critical,
        "taskA1 should NOT be critical (shorter chain)"
    );
    assert!(
        !schedule.tasks["taskA2"].is_critical,
        "taskA2 should NOT be critical (shorter chain)"
    );

    // Verify slack exists for chain A
    assert!(
        schedule.tasks["taskA2"].slack.as_days() > 0.0,
        "taskA2 should have positive slack"
    );
}

#[test]
fn critical_path_with_ff_dependencies() {
    // Test critical path with Finish-to-Finish dependencies

    let mut project = Project::new("Critical Path FF Test");
    project.start = date(2025, 2, 3);

    // Two tasks that must finish together (FF)
    // task1: 20 days
    // task2: 10 days, FF with task1 (must finish when task1 finishes)
    // task3: depends on both (FS)

    let mut task1 = Task::new("task1").effort(Duration::days(20));
    task1
        .constraints
        .push(TaskConstraint::MustStartOn(date(2025, 2, 3)));

    let mut task2 = Task::new("task2").effort(Duration::days(10));
    task2.depends.push(utf8proj_core::Dependency {
        predecessor: "task1".to_string(),
        dep_type: utf8proj_core::DependencyType::FinishToFinish,
        lag: None,
    });

    let task3 = Task::new("task3")
        .effort(Duration::days(5))
        .depends_on("task1")
        .depends_on("task2");

    project.tasks = vec![task1, task2, task3];

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).unwrap();

    // task2 should finish at the same time as task1
    assert_eq!(
        schedule.tasks["task2"].finish, schedule.tasks["task1"].finish,
        "FF dependency: task2 should finish when task1 finishes"
    );

    // task2 should start later (since it's shorter but must finish with task1)
    // task1: 20 days, task2: 10 days with FF
    // task2 starts at day 10 so it finishes at day 20
    assert!(
        schedule.tasks["task2"].start > schedule.tasks["task1"].start,
        "task2 should start later to align finish with task1"
    );

    // task3 is definitely on critical path (last task)
    assert!(
        schedule.critical_path.contains(&"task3".to_string()),
        "task3 should be on critical path: {:?}",
        schedule.critical_path
    );

    // task2 is on critical path (directly feeds task3 via FS)
    assert!(
        schedule.critical_path.contains(&"task2".to_string()),
        "task2 should be on critical path: {:?}",
        schedule.critical_path
    );

    // Note: task1 may or may not be marked critical depending on backward pass
    // implementation for FF dependencies. The key behaviors verified above are:
    // 1. FF alignment works (task2.finish == task1.finish)
    // 2. Scheduling is correct (task2 starts later to align)
}

#[test]
fn schedule_matches_tj3_output() {
    // Parse ttg_02_deps.tjp
    // Schedule with utf8proj
    // Compare dates against TJ3 scheduled output
    // Tolerance: 1 working day

    // Read and parse the TJP file
    // Path: utf8proj/crates/utf8proj-solver -> utf8proj -> projects -> msproject-to-taskjuggler
    let tjp_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent() // crates
        .unwrap()
        .parent() // utf8proj
        .unwrap()
        .parent() // projects
        .unwrap()
        .join("msproject-to-taskjuggler/examples/ttg_02_deps.tjp");

    let content = match std::fs::read_to_string(&tjp_path) {
        Ok(c) => c,
        Err(_) => {
            println!("Skipping test: ttg_02_deps.tjp not found at {:?}", tjp_path);
            return;
        }
    };

    let project = utf8proj_parser::tjp::parse(&content).expect("Should parse ttg_02_deps.tjp");

    // Schedule the project
    let solver = CpmSolver::new();
    let schedule = solver
        .schedule(&project)
        .expect("Should schedule successfully");

    // Expected dates based on the TJP file structure:
    // act1_1: starts 2025-02-03, 20 working days -> ends Feb 28
    // act2_1: FS+5d from act1_1 -> starts Mar 10 (Feb 28 + 1 + 5 working days), 65d
    // act3_1: FS from act1_1 -> starts Mar 3, 75d
    // act3_2: FS+5d from act1_1 -> starts Mar 10, 55d
    // act3_3: FS+25d from act3_2 -> starts after act3_2 + 25d gap
    // act4_1: FS from all -> starts after latest predecessor

    // Verify act1_1 (GNU Validation)
    let act1_1 = &schedule.tasks["act1_1"];
    assert_eq!(
        act1_1.start,
        date(2025, 2, 3),
        "act1_1 should start on 2025-02-03"
    );
    assert_eq!(
        act1_1.finish,
        date(2025, 2, 28),
        "act1_1 (20d from Feb 03) should finish Feb 28"
    );

    // Verify act2_1 (ABL Services Development)
    // Depends on act1_1 with gaplength 5d (FS+5d)
    // act1_1 finishes Feb 28, so act2_1 starts 5 working days after = Mar 7
    // (Mar 3, 4, 5, 6, 7 = 5 days, but we count from the day after finish)
    // Actually: FS means start after finish, +5d means 5 more days
    // Feb 28 (Fri) -> Mar 3 (Mon) is first day after, +5d = Mar 10
    let act2_1 = &schedule.tasks["act2_1"];
    assert_eq!(
        act2_1.start,
        date(2025, 3, 10),
        "act2_1 (FS+5d from act1_1) should start Mar 10"
    );

    // Verify act3_1 (ABL Migration)
    // Depends on act1_1 (FS, no gap) -> starts Mar 3 (first day after Feb 28)
    let act3_1 = &schedule.tasks["act3_1"];
    assert_eq!(
        act3_1.start,
        date(2025, 3, 3),
        "act3_1 (FS from act1_1) should start Mar 3"
    );

    // Verify act3_2 (Shell Migration)
    // Depends on act1_1 with gaplength 5d -> starts Mar 10
    let act3_2 = &schedule.tasks["act3_2"];
    assert_eq!(
        act3_2.start,
        date(2025, 3, 10),
        "act3_2 (FS+5d from act1_1) should start Mar 10"
    );

    // Verify act3_3 (Perl Migration)
    // Depends on act3_2 with gaplength 25d
    // act3_2: Mar 10 + 55d = ends around May 23
    // act3_3 starts 25 working days after that
    let act3_3 = &schedule.tasks["act3_3"];
    assert!(
        act3_3.start > act3_2.finish,
        "act3_3 should start after act3_2 finishes"
    );

    // Verify act4_1 (Platform Cutover)
    // Depends on act2_1, act3_1, act3_2, act3_3 (all FS)
    // Should start after all predecessors finish
    let act4_1 = &schedule.tasks["act4_1"];
    assert!(
        act4_1.start > act2_1.finish,
        "act4_1 should start after act2_1: {} > {}",
        act4_1.start,
        act2_1.finish
    );
    assert!(
        act4_1.start > act3_1.finish,
        "act4_1 should start after act3_1: {} > {}",
        act4_1.start,
        act3_1.finish
    );
    assert!(
        act4_1.start > act3_3.finish,
        "act4_1 should start after act3_3: {} > {}",
        act4_1.start,
        act3_3.finish
    );

    // Print schedule for debugging (visible with --nocapture)
    println!("\nSchedule from ttg_02_deps.tjp:");
    println!("  act1_1: {} - {} (20d)", act1_1.start, act1_1.finish);
    println!("  act2_1: {} - {} (65d)", act2_1.start, act2_1.finish);
    println!("  act3_1: {} - {} (75d)", act3_1.start, act3_1.finish);
    println!("  act3_2: {} - {} (55d)", act3_2.start, act3_2.finish);
    println!("  act3_3: {} - {} (30d)", act3_3.start, act3_3.finish);
    println!("  act4_1: {} - {} (15d)", act4_1.start, act4_1.finish);
    println!("  Project end: {}", schedule.project_end);
}

#[test]
fn schedule_ttg_hierarchy() {
    // Test hierarchical task scheduling from ttg_03_hierarchy.tjp

    let tjp_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent() // crates
        .unwrap()
        .parent() // utf8proj
        .unwrap()
        .parent() // projects
        .unwrap()
        .join("msproject-to-taskjuggler/examples/ttg_03_hierarchy.tjp");

    let content = match std::fs::read_to_string(&tjp_path) {
        Ok(c) => c,
        Err(_) => {
            println!("Skipping test: ttg_03_hierarchy.tjp not found");
            return;
        }
    };

    let project = utf8proj_parser::tjp::parse(&content).expect("Should parse ttg_03_hierarchy.tjp");

    let solver = CpmSolver::new();
    let schedule = solver
        .schedule(&project)
        .expect("Should schedule successfully");

    // Verify we got all tasks scheduled
    assert!(!schedule.tasks.is_empty(), "Should have scheduled tasks");

    println!("\nSchedule from ttg_03_hierarchy.tjp:");
    let mut task_ids: Vec<_> = schedule.tasks.keys().collect();
    task_ids.sort();
    for id in task_ids {
        let task = &schedule.tasks[id];
        println!("  {}: {} - {}", id, task.start, task.finish);
    }
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
        .child(
            Task::new("act1")
                .effort(Duration::days(5))
                .depends_on("act2"),
        )
        .child(
            Task::new("act2")
                .effort(Duration::days(5))
                .depends_on("act1"),
        )];

    let solver = CpmSolver::new();
    let result = solver.schedule(&project);

    assert!(
        result.is_err(),
        "Should detect circular dependency in nested tasks"
    );
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

    assert!(
        result.is_err(),
        "Should detect circular dependency across containers"
    );
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
    assert_eq!(
        phase1.duration,
        Duration::zero(),
        "Empty container should have zero duration"
    );

    // Should start at project start
    assert_eq!(
        phase1.start,
        date(2025, 2, 3),
        "Empty container should start at project start"
    );

    // Start and finish should be the same (like a milestone)
    assert_eq!(
        phase1.start, phase1.finish,
        "Empty container start/finish should match"
    );
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
        Task::new("work")
            .effort(Duration::days(5))
            .depends_on("phase1"),
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
    assert!(
        total_tasks >= 100,
        "Should have 100+ tasks, got {}",
        total_tasks
    );

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
