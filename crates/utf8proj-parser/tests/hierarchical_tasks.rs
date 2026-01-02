//! TDD Test Suite: Hierarchical Task Support
//!
//! These tests define the expected behavior for nested task parsing
//! and advanced dependency types. Run with: cargo test --test hierarchical_tasks
//!
//! Test progression:
//! 1. Basic nested task parsing
//! 2. Multi-level nesting
//! 3. Dependency path resolution
//! 4. SS/FF/SF dependency types
//! 5. Container task date derivation

use utf8proj_parser::tjp::parse;

// =============================================================================
// Phase 1: Nested Task Parsing
// =============================================================================

#[test]
fn parse_single_nested_task() {
    let input = r#"
project test "Test" 2025-01-01 - 2025-12-31 {
    timezone "UTC"
}

task phase1 "Phase 1" {
    task act1 "Activity 1" {
        length 10d
    }
}
"#;
    let project = parse(input).expect("Should parse nested task");

    assert_eq!(project.tasks.len(), 1, "Should have 1 top-level task");
    // TODO: Access subtasks - requires Task struct enhancement
    // assert_eq!(project.tasks[0].subtasks.len(), 1);
}

#[test]
fn parse_multiple_nested_tasks() {
    let input = r#"
project test "Test" 2025-01-01 - 2025-12-31 {
    timezone "UTC"
}

task phase1 "Phase 1" {
    task act1 "Activity 1" { length 10d }
    task act2 "Activity 2" { length 5d }
    task act3 "Activity 3" { length 8d }
}
"#;
    let project = parse(input).expect("Should parse multiple nested tasks");

    assert_eq!(project.tasks.len(), 1);
    // assert_eq!(project.tasks[0].subtasks.len(), 3);
}

#[test]
fn parse_3_level_nesting() {
    let input = r#"
project test "Test" 2025-01-01 - 2025-12-31 {
    timezone "UTC"
}

task phase1 "Phase 1" {
    task act1 "Activity 1" {
        task sub1 "Sub-task 1" { length 5d }
        task sub2 "Sub-task 2" { length 3d }
    }
}
"#;
    let project = parse(input).expect("Should parse 3-level nesting");

    assert_eq!(project.tasks.len(), 1);
    // assert_eq!(project.tasks[0].subtasks[0].subtasks.len(), 2);
}

#[test]
fn parse_milestone_in_container() {
    let input = r#"
project test "Test" 2025-01-01 - 2025-12-31 {
    timezone "UTC"
}

task phase1 "Phase 1" {
    task act1 "Activity 1" { length 10d }
    task gate1 "Gate 1" {
        milestone
        depends phase1.act1
    }
}
"#;
    let project = parse(input).expect("Should parse milestone in container");

    // Milestone should be parsed as a subtask
    // assert!(project.tasks[0].subtasks[1].is_milestone);
}

// =============================================================================
// Phase 2: Dependency Path Resolution
// =============================================================================

#[test]
#[ignore = "Phase 2: Not yet implemented"]
fn resolve_sibling_dependency() {
    let input = r#"
project test "Test" 2025-01-01 - 2025-12-31 {
    timezone "UTC"
}

task phase1 "Phase 1" {
    task act1 "Activity 1" { length 10d }
    task act2 "Activity 2" {
        depends act1
        length 5d
    }
}
"#;
    let project = parse(input).expect("Should parse sibling dependency");

    // Dependency should resolve to sibling within same container
    // let dep = &project.tasks[0].subtasks[1].dependencies[0];
    // assert_eq!(dep.resolved_target, "phase1.act1");
}

#[test]
#[ignore = "Phase 2: Not yet implemented"]
fn resolve_absolute_path_dependency() {
    let input = r#"
project test "Test" 2025-01-01 - 2025-12-31 {
    timezone "UTC"
}

task phase1 "Phase 1" {
    task act1 "Activity 1" { length 10d }
}

task phase2 "Phase 2" {
    depends phase1.act1
    task act2 "Activity 2" { length 5d }
}
"#;
    let project = parse(input).expect("Should parse absolute path dependency");

    // Container dependency should reference task in another container
    // let dep = &project.tasks[1].dependencies[0];
    // assert_eq!(dep.target_path, vec!["phase1", "act1"]);
}

#[test]
#[ignore = "Phase 2: Not yet implemented"]
fn resolve_cross_container_dependency() {
    let input = r#"
project test "Test" 2025-01-01 - 2025-12-31 {
    timezone "UTC"
}

task phase1 "Phase 1" {
    task act1 "Activity 1" { length 10d }
}

task phase2 "Phase 2" {
    task act2 "Activity 2" {
        depends phase1.act1
        length 5d
    }
}
"#;
    let project = parse(input).expect("Should parse cross-container dependency");

    // Nested task references task in different container
}

// =============================================================================
// Phase 3: Container Task Date Derivation
// =============================================================================

#[test]
#[ignore = "Phase 3: Requires scheduling integration"]
fn container_start_is_min_of_children() {
    // Container start = earliest child start
    // This requires scheduler integration
}

#[test]
#[ignore = "Phase 3: Requires scheduling integration"]
fn container_finish_is_max_of_children() {
    // Container finish = latest child finish
    // This requires scheduler integration
}

// =============================================================================
// Phase 4: Advanced Dependency Types
// =============================================================================

#[test]
#[ignore = "Phase 4: Not yet implemented"]
fn parse_ss_dependency() {
    let input = r#"
project test "Test" 2025-01-01 - 2025-12-31 {
    timezone "UTC"
}

task act1 "Activity 1" { start 2025-02-03 length 20d }
task act2 "Activity 2" {
    depends !act1
    length 10d
}
"#;
    let project = parse(input).expect("Should parse SS dependency");

    // Check that dependency is marked as Start-to-Start
    // let dep = &project.tasks[1].dependencies[0];
    // assert!(dep.is_start_to_start());
}

#[test]
#[ignore = "Phase 4: Not yet implemented"]
fn parse_ss_dependency_with_lag() {
    let input = r#"
project test "Test" 2025-01-01 - 2025-12-31 {
    timezone "UTC"
}

task act1 "Activity 1" { start 2025-02-03 length 20d }
task act2 "Activity 2" {
    depends !act1 { gaplength 5d }
    length 10d
}
"#;
    let project = parse(input).expect("Should parse SS+lag dependency");

    // SS + 5 day lag
    // let dep = &project.tasks[1].dependencies[0];
    // assert!(dep.is_start_to_start());
    // assert_eq!(dep.lag_days, 5);
}

#[test]
#[ignore = "Phase 4: Not yet implemented"]
fn parse_ff_dependency() {
    let input = r#"
project test "Test" 2025-01-01 - 2025-12-31 {
    timezone "UTC"
}

task act1 "Activity 1" { start 2025-02-03 length 20d }
task act2 "Activity 2" {
    depends act1~
    length 10d
}
"#;
    let project = parse(input).expect("Should parse FF dependency");

    // Check that dependency is marked as Finish-to-Finish
    // let dep = &project.tasks[1].dependencies[0];
    // assert!(dep.is_finish_to_finish());
}

#[test]
#[ignore = "Phase 4: Not yet implemented"]
fn parse_sf_dependency() {
    let input = r#"
project test "Test" 2025-01-01 - 2025-12-31 {
    timezone "UTC"
}

task act1 "Activity 1" { start 2025-02-03 length 20d }
task act2 "Activity 2" {
    depends !act1~
    length 10d
}
"#;
    let project = parse(input).expect("Should parse SF dependency");

    // Check that dependency is marked as Start-to-Finish
    // let dep = &project.tasks[1].dependencies[0];
    // assert!(dep.is_start_to_finish());
}

// =============================================================================
// Phase 5: Negative Lag (Lead)
// =============================================================================

#[test]
#[ignore = "Phase 5: Not yet implemented"]
fn parse_negative_lag() {
    let input = r#"
project test "Test" 2025-01-01 - 2025-12-31 {
    timezone "UTC"
}

task act1 "Activity 1" { start 2025-02-03 length 20d }
task act2 "Activity 2" {
    depends act1 { gaplength -5d }
    length 10d
}
"#;
    // Note: TJ3 doesn't support negative gaplength directly,
    // but we should support it for MS Project compatibility
    let project = parse(input).expect("Should parse negative lag");

    // let dep = &project.tasks[1].dependencies[0];
    // assert_eq!(dep.lag_days, -5);
}

// =============================================================================
// Integration Tests: Full File Parsing
// =============================================================================

#[test]
fn parse_ttg_01_base() {
    let content = std::fs::read_to_string(
        "../../msproject-to-taskjuggler/examples/ttg_01_base.tjp"
    );

    if let Ok(content) = content {
        let result = parse(&content);
        assert!(result.is_ok(), "ttg_01_base.tjp should parse: {:?}", result.err());
    }
}

#[test]
fn parse_ttg_02_deps() {
    let content = std::fs::read_to_string(
        "../../msproject-to-taskjuggler/examples/ttg_02_deps.tjp"
    );

    if let Ok(content) = content {
        let result = parse(&content);
        assert!(result.is_ok(), "ttg_02_deps.tjp should parse: {:?}", result.err());
    }
}

#[test]
fn parse_ttg_03_hierarchy() {
    let content = std::fs::read_to_string(
        "../../msproject-to-taskjuggler/examples/ttg_03_hierarchy.tjp"
    );

    if let Ok(content) = content {
        let result = parse(&content);
        assert!(result.is_ok(), "ttg_03_hierarchy.tjp should parse: {:?}", result.err());
    }
}

#[test]
fn parse_ttg_04_milestones() {
    let content = std::fs::read_to_string(
        "../../msproject-to-taskjuggler/examples/ttg_04_milestones.tjp"
    );

    if let Ok(content) = content {
        let result = parse(&content);
        assert!(result.is_ok(), "ttg_04_milestones.tjp should parse: {:?}", result.err());
    }
}

#[test]
fn parse_ttg_05_detailed() {
    let content = std::fs::read_to_string(
        "../../msproject-to-taskjuggler/examples/ttg_05_detailed.tjp"
    );

    if let Ok(content) = content {
        let result = parse(&content);
        assert!(result.is_ok(), "ttg_05_detailed.tjp should parse: {:?}", result.err());
    }
}
