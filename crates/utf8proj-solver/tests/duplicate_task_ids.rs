use utf8proj_core::{CollectingEmitter, DiagnosticCode, Duration, Project, Task};
use utf8proj_solver::{analyze_project, AnalysisConfig};

#[test]
fn duplicate_ids_at_root_emits_e004() {
    let mut project = Project::new("Test");
    project
        .tasks
        .push(Task::new("alpha").effort(Duration::days(3)));
    project
        .tasks
        .push(Task::new("alpha").effort(Duration::days(5)));

    let mut emitter = CollectingEmitter::new();
    let config = AnalysisConfig::default();
    analyze_project(&project, None, &config, &mut emitter);

    let e004: Vec<_> = emitter
        .diagnostics
        .iter()
        .filter(|d| d.code == DiagnosticCode::E004DuplicateTaskId)
        .collect();
    assert_eq!(e004.len(), 1);
    assert!(e004[0].message.contains("alpha"));
}

#[test]
fn duplicate_ids_in_container_emits_e004() {
    let mut project = Project::new("Test");
    let mut container = Task::new("phase1");
    container
        .children
        .push(Task::new("child").effort(Duration::days(2)));
    container
        .children
        .push(Task::new("child").effort(Duration::days(4)));
    project.tasks.push(container);

    let mut emitter = CollectingEmitter::new();
    let config = AnalysisConfig::default();
    analyze_project(&project, None, &config, &mut emitter);

    let e004: Vec<_> = emitter
        .diagnostics
        .iter()
        .filter(|d| d.code == DiagnosticCode::E004DuplicateTaskId)
        .collect();
    assert_eq!(e004.len(), 1);
    assert!(e004[0].message.contains("child"));
}

#[test]
fn same_id_in_different_containers_no_e004() {
    let mut project = Project::new("Test");

    let mut container1 = Task::new("phase1");
    container1
        .children
        .push(Task::new("setup").effort(Duration::days(2)));
    project.tasks.push(container1);

    let mut container2 = Task::new("phase2");
    container2
        .children
        .push(Task::new("setup").effort(Duration::days(3)));
    project.tasks.push(container2);

    let mut emitter = CollectingEmitter::new();
    let config = AnalysisConfig::default();
    analyze_project(&project, None, &config, &mut emitter);

    let count = emitter
        .diagnostics
        .iter()
        .filter(|d| d.code == DiagnosticCode::E004DuplicateTaskId)
        .count();
    assert_eq!(count, 0);
}

#[test]
fn no_duplicates_no_e004() {
    let mut project = Project::new("Test");
    project
        .tasks
        .push(Task::new("task_a").effort(Duration::days(3)));
    project
        .tasks
        .push(Task::new("task_b").effort(Duration::days(5)));

    let mut emitter = CollectingEmitter::new();
    let config = AnalysisConfig::default();
    analyze_project(&project, None, &config, &mut emitter);

    let count = emitter
        .diagnostics
        .iter()
        .filter(|d| d.code == DiagnosticCode::E004DuplicateTaskId)
        .count();
    assert_eq!(count, 0);
}

#[test]
fn triple_duplicate_emits_e004_once() {
    let mut project = Project::new("Test");
    project
        .tasks
        .push(Task::new("dup").effort(Duration::days(1)));
    project
        .tasks
        .push(Task::new("dup").effort(Duration::days(2)));
    project
        .tasks
        .push(Task::new("dup").effort(Duration::days(3)));

    let mut emitter = CollectingEmitter::new();
    let config = AnalysisConfig::default();
    analyze_project(&project, None, &config, &mut emitter);

    let count = emitter
        .diagnostics
        .iter()
        .filter(|d| d.code == DiagnosticCode::E004DuplicateTaskId)
        .count();
    assert_eq!(count, 1, "should emit E004 exactly once for triple duplicate");
}
