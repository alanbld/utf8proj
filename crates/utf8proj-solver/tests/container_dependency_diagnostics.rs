//! Tests for W014: Container Dependency Diagnostics and Fix
//!
//! These tests verify that utf8proj correctly detects when container tasks
//! have dependencies but their children don't have matching dependencies,
//! and that the fix_container_dependencies function correctly propagates them.

use utf8proj_core::{
    Dependency, DependencyType, Diagnostic, DiagnosticCode, DiagnosticEmitter, Duration, Project,
    Task,
};
use utf8proj_solver::{analyze_project, fix_container_dependencies, AnalysisConfig};

/// Simple diagnostic collector for testing
struct TestEmitter {
    diagnostics: Vec<Diagnostic>,
}

impl TestEmitter {
    fn new() -> Self {
        Self {
            diagnostics: Vec::new(),
        }
    }

    fn count(&self, code: DiagnosticCode) -> usize {
        self.diagnostics.iter().filter(|d| d.code == code).count()
    }

    fn has(&self, code: DiagnosticCode) -> bool {
        self.diagnostics.iter().any(|d| d.code == code)
    }
}

impl DiagnosticEmitter for TestEmitter {
    fn emit(&mut self, diagnostic: Diagnostic) {
        self.diagnostics.push(diagnostic);
    }
}

/// Helper to create a dependency
fn dep(predecessor: &str) -> Dependency {
    Dependency {
        predecessor: predecessor.to_string(),
        dep_type: DependencyType::FinishToStart,
        lag: None,
    }
}

/// Helper to create a task with dependencies
fn task_with_deps(id: &str, deps: &[&str]) -> Task {
    let mut task = Task::new(id).effort(Duration::days(3));
    for d in deps {
        task.depends.push(dep(d));
    }
    task
}

/// Helper to create a container with children
fn container_with_children(id: &str, deps: &[&str], children: Vec<Task>) -> Task {
    let mut task = Task::new(id);
    for d in deps {
        task.depends.push(dep(d));
    }
    task.children = children;
    task
}

#[test]
fn w014_triggers_when_child_missing_container_dep() {
    let mut project = Project::new("test");

    // Add predecessor task
    project
        .tasks
        .push(Task::new("predecessor").effort(Duration::days(5)));

    // Add container with dependency but child without
    let child = Task::new("child").effort(Duration::days(3));
    let container = container_with_children("container", &["predecessor"], vec![child]);
    project.tasks.push(container);

    let config = AnalysisConfig::default();
    let mut emitter = TestEmitter::new();
    analyze_project(&project, None, &config, &mut emitter);

    assert!(emitter.has(DiagnosticCode::W014ContainerDependency));
    assert_eq!(emitter.count(DiagnosticCode::W014ContainerDependency), 1);
}

#[test]
fn w014_does_not_trigger_when_child_has_dep() {
    let mut project = Project::new("test");

    // Add predecessor task
    project
        .tasks
        .push(Task::new("predecessor").effort(Duration::days(5)));

    // Add container with dependency and child with same dependency
    let child = task_with_deps("child", &["predecessor"]);
    let container = container_with_children("container", &["predecessor"], vec![child]);
    project.tasks.push(container);

    let config = AnalysisConfig::default();
    let mut emitter = TestEmitter::new();
    analyze_project(&project, None, &config, &mut emitter);

    assert!(!emitter.has(DiagnosticCode::W014ContainerDependency));
}

#[test]
fn w014_triggers_for_each_missing_child() {
    let mut project = Project::new("test");

    // Add predecessor task
    project
        .tasks
        .push(Task::new("predecessor").effort(Duration::days(5)));

    // Container with 3 children: 1 has dep, 2 don't
    let child1 = Task::new("child1").effort(Duration::days(3));
    let child2 = task_with_deps("child2", &["predecessor"]);
    let child3 = Task::new("child3").effort(Duration::days(2));

    let container = container_with_children("container", &["predecessor"], vec![child1, child2, child3]);
    project.tasks.push(container);

    let config = AnalysisConfig::default();
    let mut emitter = TestEmitter::new();
    analyze_project(&project, None, &config, &mut emitter);

    // Should trigger for child1 and child3 (child2 has the dependency)
    assert_eq!(emitter.count(DiagnosticCode::W014ContainerDependency), 2);
}

#[test]
fn w014_does_not_trigger_for_empty_container_deps() {
    let mut project = Project::new("test");

    // Container without dependencies
    let child = Task::new("child").effort(Duration::days(3));
    let mut container = Task::new("container");
    container.children.push(child);
    project.tasks.push(container);

    let config = AnalysisConfig::default();
    let mut emitter = TestEmitter::new();
    analyze_project(&project, None, &config, &mut emitter);

    assert!(!emitter.has(DiagnosticCode::W014ContainerDependency));
}

#[test]
fn w014_does_not_trigger_for_leaf_tasks() {
    let mut project = Project::new("test");

    // Add predecessor task
    project
        .tasks
        .push(Task::new("predecessor").effort(Duration::days(5)));

    // Leaf task with dependency (not a container)
    let leaf = task_with_deps("leaf", &["predecessor"]);
    project.tasks.push(leaf);

    let config = AnalysisConfig::default();
    let mut emitter = TestEmitter::new();
    analyze_project(&project, None, &config, &mut emitter);

    assert!(!emitter.has(DiagnosticCode::W014ContainerDependency));
}

#[test]
fn w014_nested_containers() {
    let mut project = Project::new("test");

    // Add predecessor tasks
    project
        .tasks
        .push(Task::new("dep_outer").effort(Duration::days(5)));
    project
        .tasks
        .push(Task::new("dep_inner").effort(Duration::days(3)));

    // Nested structure:
    // outer (depends: dep_outer)
    //   inner (depends: dep_inner)
    //     leaf (no deps) <- should trigger W014 for inner's dep
    let leaf = Task::new("leaf").effort(Duration::days(2));
    let inner = container_with_children("inner", &["dep_inner"], vec![leaf]);
    let outer = container_with_children("outer", &["dep_outer"], vec![inner]);
    project.tasks.push(outer);

    let config = AnalysisConfig::default();
    let mut emitter = TestEmitter::new();
    analyze_project(&project, None, &config, &mut emitter);

    // Should trigger for:
    // 1. outer's child (inner) doesn't have dep_outer
    // 2. inner's child (leaf) doesn't have dep_inner
    assert_eq!(emitter.count(DiagnosticCode::W014ContainerDependency), 2);
}

#[test]
fn w014_message_includes_task_names() {
    let mut project = Project::new("test");

    // Add predecessor task
    project.tasks.push(
        Task::new("design_approval")
            .name("Design Approval")
            .effort(Duration::days(5)),
    );

    // Container and child
    let child = Task::new("feature_x")
        .name("Feature X")
        .effort(Duration::days(3));
    let mut container = Task::new("development").name("Development Phase");
    container.depends.push(dep("design_approval"));
    container.children.push(child);
    project.tasks.push(container);

    let config = AnalysisConfig::default();
    let mut emitter = TestEmitter::new();
    analyze_project(&project, None, &config, &mut emitter);

    let w014 = emitter
        .diagnostics
        .iter()
        .find(|d| d.code == DiagnosticCode::W014ContainerDependency)
        .expect("W014 should be emitted");

    assert!(w014.message.contains("Development Phase"));
    assert!(w014.message.contains("Feature X"));
    assert!(w014.message.contains("design_approval"));
}

// =============================================================================
// Fix Tests
// =============================================================================

#[test]
fn fix_adds_missing_container_dep() {
    let mut project = Project::new("test");

    // Add predecessor task
    project
        .tasks
        .push(Task::new("predecessor").effort(Duration::days(5)));

    // Add container with dependency but child without
    let child = Task::new("child").effort(Duration::days(3));
    let container = container_with_children("container", &["predecessor"], vec![child]);
    project.tasks.push(container);

    // Verify W014 is triggered before fix
    let config = AnalysisConfig::default();
    let mut emitter = TestEmitter::new();
    analyze_project(&project, None, &config, &mut emitter);
    assert!(emitter.has(DiagnosticCode::W014ContainerDependency));

    // Apply fix
    let fixed_count = fix_container_dependencies(&mut project);
    assert_eq!(fixed_count, 1);

    // Verify W014 is no longer triggered
    let mut emitter = TestEmitter::new();
    analyze_project(&project, None, &config, &mut emitter);
    assert!(!emitter.has(DiagnosticCode::W014ContainerDependency));

    // Verify the child now has the dependency
    let container = &project.tasks[1];
    let child = &container.children[0];
    assert_eq!(child.depends.len(), 1);
    assert_eq!(child.depends[0].predecessor, "predecessor");
}

#[test]
fn fix_is_idempotent() {
    let mut project = Project::new("test");

    // Add predecessor task
    project
        .tasks
        .push(Task::new("predecessor").effort(Duration::days(5)));

    // Add container with dependency but child without
    let child = Task::new("child").effort(Duration::days(3));
    let container = container_with_children("container", &["predecessor"], vec![child]);
    project.tasks.push(container);

    // Apply fix twice
    let fixed_count_1 = fix_container_dependencies(&mut project);
    let fixed_count_2 = fix_container_dependencies(&mut project);

    // First fix should add 1 dependency
    assert_eq!(fixed_count_1, 1);
    // Second fix should add 0 (already fixed)
    assert_eq!(fixed_count_2, 0);

    // Child should still have exactly 1 dependency
    let container = &project.tasks[1];
    let child = &container.children[0];
    assert_eq!(child.depends.len(), 1);
}

#[test]
fn fix_handles_multiple_container_deps() {
    let mut project = Project::new("test");

    // Add two predecessor tasks
    project.tasks.push(Task::new("pred1").effort(Duration::days(5)));
    project.tasks.push(Task::new("pred2").effort(Duration::days(3)));

    // Container with two dependencies
    let child = Task::new("child").effort(Duration::days(3));
    let container = container_with_children("container", &["pred1", "pred2"], vec![child]);
    project.tasks.push(container);

    // Apply fix
    let fixed_count = fix_container_dependencies(&mut project);
    assert_eq!(fixed_count, 2); // Both dependencies added

    // Verify child has both dependencies
    let container = &project.tasks[2];
    let child = &container.children[0];
    assert_eq!(child.depends.len(), 2);

    let dep_ids: Vec<_> = child.depends.iter().map(|d| d.predecessor.as_str()).collect();
    assert!(dep_ids.contains(&"pred1"));
    assert!(dep_ids.contains(&"pred2"));
}

#[test]
fn fix_preserves_existing_child_deps() {
    let mut project = Project::new("test");

    // Add predecessor tasks
    project.tasks.push(Task::new("pred1").effort(Duration::days(5)));
    project.tasks.push(Task::new("pred2").effort(Duration::days(3)));

    // Child already has pred2 dependency
    let child = task_with_deps("child", &["pred2"]);
    let container = container_with_children("container", &["pred1"], vec![child]);
    project.tasks.push(container);

    // Apply fix
    let fixed_count = fix_container_dependencies(&mut project);
    assert_eq!(fixed_count, 1); // Only pred1 added

    // Verify child has both dependencies
    let container = &project.tasks[2];
    let child = &container.children[0];
    assert_eq!(child.depends.len(), 2);

    let dep_ids: Vec<_> = child.depends.iter().map(|d| d.predecessor.as_str()).collect();
    assert!(dep_ids.contains(&"pred1"));
    assert!(dep_ids.contains(&"pred2"));
}

#[test]
fn fix_handles_nested_containers() {
    let mut project = Project::new("test");

    // Add predecessor tasks
    project.tasks.push(Task::new("dep_outer").effort(Duration::days(5)));
    project.tasks.push(Task::new("dep_inner").effort(Duration::days(3)));

    // Nested structure:
    // outer (depends: dep_outer)
    //   inner (depends: dep_inner)
    //     leaf (no deps) <- should get dep_inner AND dep_outer (inherited)
    let leaf = Task::new("leaf").effort(Duration::days(2));
    let inner = container_with_children("inner", &["dep_inner"], vec![leaf]);
    let outer = container_with_children("outer", &["dep_outer"], vec![inner]);
    project.tasks.push(outer);

    // Apply fix
    let fixed_count = fix_container_dependencies(&mut project);
    // outer->inner gets dep_outer (+1)
    // inner->leaf gets dep_inner (+1)
    // inner->leaf also gets dep_outer (because inner now has it) (+1)
    assert_eq!(fixed_count, 3);

    // Verify inner has dep_outer (propagated from outer)
    let outer = &project.tasks[2];
    let inner = &outer.children[0];
    assert!(inner.depends.iter().any(|d| d.predecessor == "dep_outer"));

    // Verify leaf has both dep_inner and dep_outer
    let leaf = &inner.children[0];
    assert!(leaf.depends.iter().any(|d| d.predecessor == "dep_inner"));
    assert!(leaf.depends.iter().any(|d| d.predecessor == "dep_outer"));
}

#[test]
fn fix_w014_count_goes_to_zero() {
    let mut project = Project::new("test");

    // Add predecessor
    project.tasks.push(Task::new("predecessor").effort(Duration::days(5)));

    // Container with 3 children, none with the dependency
    let child1 = Task::new("child1").effort(Duration::days(3));
    let child2 = Task::new("child2").effort(Duration::days(2));
    let child3 = Task::new("child3").effort(Duration::days(4));
    let container = container_with_children("container", &["predecessor"], vec![child1, child2, child3]);
    project.tasks.push(container);

    // Count W014 before fix
    let config = AnalysisConfig::default();
    let mut emitter = TestEmitter::new();
    analyze_project(&project, None, &config, &mut emitter);
    let w014_before = emitter.count(DiagnosticCode::W014ContainerDependency);
    assert_eq!(w014_before, 3);

    // Apply fix
    let fixed_count = fix_container_dependencies(&mut project);
    assert_eq!(fixed_count, 3);

    // Count W014 after fix
    let mut emitter = TestEmitter::new();
    analyze_project(&project, None, &config, &mut emitter);
    let w014_after = emitter.count(DiagnosticCode::W014ContainerDependency);
    assert_eq!(w014_after, 0);
}
