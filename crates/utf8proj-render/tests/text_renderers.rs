//! Tests for Mermaid and PlantUML text renderers

use chrono::NaiveDate;
use utf8proj_core::{Duration, Project, Resource, Renderer, Scheduler, Task};
use utf8proj_render::{MermaidRenderer, PlantUmlRenderer};
use utf8proj_solver::CpmSolver;

fn date(year: i32, month: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(year, month, day).unwrap()
}

// ============================================================================
// Mermaid Renderer Tests
// ============================================================================

#[test]
fn mermaid_basic_project() {
    let mut project = Project::new("Mermaid Test");
    project.start = date(2025, 1, 6);
    project.tasks = vec![
        Task::new("task1").name("Task One").duration(Duration::days(5)),
        Task::new("task2").name("Task Two").duration(Duration::days(3)).depends_on("task1"),
    ];

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).unwrap();

    let renderer = MermaidRenderer::new();
    let mermaid = renderer.render(&project, &schedule).unwrap();

    assert!(mermaid.contains("gantt"));
    assert!(mermaid.contains("title Mermaid Test"));
    assert!(mermaid.contains("Task One"));
    assert!(mermaid.contains("Task Two"));
}

#[test]
fn mermaid_with_milestones() {
    let mut project = Project::new("Milestone Project");
    project.start = date(2025, 1, 6);
    project.tasks = vec![
        Task::new("dev").name("Development").duration(Duration::days(10)),
        Task::new("release").name("Release").milestone().depends_on("dev"),
    ];

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).unwrap();

    let renderer = MermaidRenderer::new();
    let mermaid = renderer.render(&project, &schedule).unwrap();

    assert!(mermaid.contains("Development"));
    assert!(mermaid.contains("Release"));
    // Milestones should have 0d duration
    assert!(mermaid.contains("0d"));
}

#[test]
fn mermaid_with_sections() {
    let mut project = Project::new("Sectioned Project");
    project.start = date(2025, 1, 6);
    project.tasks = vec![
        Task::new("phase1").name("Phase 1")
            .child(Task::new("a").name("Task A").duration(Duration::days(5)))
            .child(Task::new("b").name("Task B").duration(Duration::days(3)).depends_on("a")),
        Task::new("phase2").name("Phase 2")
            .child(Task::new("c").name("Task C").duration(Duration::days(4)).depends_on("phase1.b")),
    ];

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).unwrap();

    let renderer = MermaidRenderer::new();
    let mermaid = renderer.render(&project, &schedule).unwrap();

    // Mermaid output has basic structure
    assert!(mermaid.contains("gantt"));
    assert!(mermaid.contains("title"));
    assert!(mermaid.contains("dateFormat"));
    // Should contain scheduled tasks
    assert!(!mermaid.is_empty());
    assert!(mermaid.len() > 100); // Non-trivial output
}

#[test]
fn mermaid_with_critical_path() {
    let mut project = Project::new("Critical Path");
    project.start = date(2025, 1, 6);
    project.tasks = vec![
        Task::new("a").name("Task A").duration(Duration::days(5)),
        Task::new("b").name("Task B").duration(Duration::days(3)).depends_on("a"),
        Task::new("c").name("Task C").duration(Duration::days(2)).depends_on("b"),
    ];

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).unwrap();

    let renderer = MermaidRenderer::new();
    let mermaid = renderer.render(&project, &schedule).unwrap();

    // Critical tasks should be marked
    assert!(mermaid.contains("crit"));
}

#[test]
fn mermaid_with_resources() {
    let mut project = Project::new("Resource Project");
    project.start = date(2025, 1, 6);
    project.resources = vec![
        Resource::new("dev").name("Developer"),
        Resource::new("qa").name("QA"),
    ];
    project.tasks = vec![
        Task::new("impl").name("Implementation").duration(Duration::days(5)).assign("dev"),
        Task::new("test").name("Testing").duration(Duration::days(3)).depends_on("impl").assign("qa"),
    ];

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).unwrap();

    let renderer = MermaidRenderer::new();
    let mermaid = renderer.render(&project, &schedule).unwrap();

    assert!(mermaid.contains("Implementation"));
    assert!(mermaid.contains("Testing"));
}

// ============================================================================
// PlantUML Renderer Tests
// ============================================================================

#[test]
fn plantuml_basic_project() {
    let mut project = Project::new("PlantUML Test");
    project.start = date(2025, 1, 6);
    project.tasks = vec![
        Task::new("task1").name("Task One").duration(Duration::days(5)),
        Task::new("task2").name("Task Two").duration(Duration::days(3)).depends_on("task1"),
    ];

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).unwrap();

    let renderer = PlantUmlRenderer::new();
    let plantuml = renderer.render(&project, &schedule).unwrap();

    assert!(plantuml.contains("@startgantt"));
    assert!(plantuml.contains("@endgantt"));
    assert!(plantuml.contains("Task One"));
    assert!(plantuml.contains("Task Two"));
}

#[test]
fn plantuml_with_milestones() {
    let mut project = Project::new("Milestone Project");
    project.start = date(2025, 1, 6);
    project.tasks = vec![
        Task::new("dev").name("Development").duration(Duration::days(10)),
        Task::new("release").name("Release").milestone().depends_on("dev"),
    ];

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).unwrap();

    let renderer = PlantUmlRenderer::new();
    let plantuml = renderer.render(&project, &schedule).unwrap();

    assert!(plantuml.contains("Development"));
    assert!(plantuml.contains("Release"));
}

#[test]
fn plantuml_with_dependencies() {
    let mut project = Project::new("Dependencies");
    project.start = date(2025, 1, 6);
    project.tasks = vec![
        Task::new("a").name("Task A").duration(Duration::days(5)),
        Task::new("b").name("Task B").duration(Duration::days(5)),
        Task::new("c").name("Task C").duration(Duration::days(3)).depends_on("a").depends_on("b"),
    ];

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).unwrap();

    let renderer = PlantUmlRenderer::new();
    let plantuml = renderer.render(&project, &schedule).unwrap();

    // Should show dependencies
    assert!(plantuml.contains("starts at"));
}

#[test]
fn plantuml_with_progress() {
    let mut project = Project::new("Progress Project");
    project.start = date(2025, 1, 6);
    project.tasks = vec![
        Task::new("done").name("Completed").duration(Duration::days(5)).complete(100.0),
        Task::new("half").name("Half Done").duration(Duration::days(5)).complete(50.0).depends_on("done"),
        Task::new("pending").name("Pending").duration(Duration::days(5)).depends_on("half"),
    ];

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).unwrap();

    let renderer = PlantUmlRenderer::new();
    let plantuml = renderer.render(&project, &schedule).unwrap();

    assert!(plantuml.contains("Completed"));
    assert!(plantuml.contains("Half Done"));
}

#[test]
fn plantuml_with_hierarchical_tasks() {
    let mut project = Project::new("Hierarchical");
    project.start = date(2025, 1, 6);
    project.tasks = vec![
        Task::new("phase1").name("Phase 1")
            .child(Task::new("a").name("Task A").duration(Duration::days(5)))
            .child(Task::new("b").name("Task B").duration(Duration::days(3)).depends_on("a")),
        Task::new("phase2").name("Phase 2")
            .child(Task::new("c").name("Task C").duration(Duration::days(4)).depends_on("phase1.b")),
    ];

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).unwrap();

    let renderer = PlantUmlRenderer::new();
    let plantuml = renderer.render(&project, &schedule).unwrap();

    // PlantUML output has basic structure
    assert!(plantuml.contains("@startgantt"));
    assert!(plantuml.contains("@endgantt"));
    // Should contain scheduled tasks (not empty output)
    assert!(!plantuml.is_empty());
    assert!(plantuml.len() > 50);
}

#[test]
fn plantuml_long_project() {
    // Test with longer project to ensure dates are formatted correctly
    let mut project = Project::new("Long Project");
    project.start = date(2025, 1, 6);
    project.tasks = vec![
        Task::new("q1").name("Q1 Work").duration(Duration::days(60)),
        Task::new("q2").name("Q2 Work").duration(Duration::days(60)).depends_on("q1"),
    ];

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).unwrap();

    let renderer = PlantUmlRenderer::new();
    let plantuml = renderer.render(&project, &schedule).unwrap();

    assert!(plantuml.contains("Q1 Work"));
    assert!(plantuml.contains("Q2 Work"));
    // Should have project start date
    assert!(plantuml.contains("2025"));
}
