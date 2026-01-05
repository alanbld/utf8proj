//! Integration tests for HTML Gantt chart rendering

use chrono::NaiveDate;
use utf8proj_core::{Duration, Project, Resource, Renderer, Scheduler, Task};
use utf8proj_render::HtmlGanttRenderer;
use utf8proj_solver::CpmSolver;

fn date(year: i32, month: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(year, month, day).unwrap()
}

#[test]
fn render_complete_project() {
    // Create a realistic project
    let mut project = Project::new("Software Development Project");
    project.start = date(2025, 1, 6);

    project.resources = vec![
        Resource::new("dev1").name("Alice Developer"),
        Resource::new("dev2").name("Bob Engineer"),
        Resource::new("qa").name("QA Team"),
    ];

    project.tasks = vec![
        // Phase 1: Planning
        Task::new("planning")
            .name("Planning Phase")
            .child(
                Task::new("requirements")
                    .name("Gather Requirements")
                    .effort(Duration::days(5))
                    .assign("dev1"),
            )
            .child(
                Task::new("design")
                    .name("System Design")
                    .effort(Duration::days(8))
                    .depends_on("requirements")
                    .assign("dev1"),
            ),
        // Phase 2: Development
        Task::new("development")
            .name("Development Phase")
            .child(
                Task::new("backend")
                    .name("Backend Development")
                    .effort(Duration::days(15))
                    .depends_on("planning.design")
                    .assign("dev1"),
            )
            .child(
                Task::new("frontend")
                    .name("Frontend Development")
                    .effort(Duration::days(12))
                    .depends_on("planning.design")
                    .assign("dev2"),
            )
            .child(
                Task::new("integration")
                    .name("Integration")
                    .effort(Duration::days(5))
                    .depends_on("backend")
                    .depends_on("frontend")
                    .assign("dev1"),
            ),
        // Phase 3: Testing
        Task::new("testing")
            .name("Testing Phase")
            .child(
                Task::new("unit_tests")
                    .name("Unit Testing")
                    .effort(Duration::days(5))
                    .depends_on("development.integration")
                    .assign("qa"),
            )
            .child(
                Task::new("integration_tests")
                    .name("Integration Testing")
                    .effort(Duration::days(5))
                    .depends_on("unit_tests")
                    .assign("qa"),
            ),
        // Release milestone
        Task::new("release")
            .name("Release v1.0")
            .milestone()
            .depends_on("testing.integration_tests"),
    ];

    // Schedule the project
    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).unwrap();

    // Render to HTML
    let renderer = HtmlGanttRenderer::new().chart_width(1200);
    let html = renderer.render(&project, &schedule).unwrap();

    // Verify HTML structure
    assert!(html.contains("<!DOCTYPE html>"));
    assert!(html.contains("Software Development Project"));
    assert!(html.contains("Planning Phase"));
    assert!(html.contains("Backend Development"));
    assert!(html.contains("Release v1.0"));

    // Verify it contains SVG
    assert!(html.contains("<svg"));
    assert!(html.contains("</svg>"));

    // Verify it contains interactivity
    assert!(html.contains("zoomIn"));
    assert!(html.contains("tooltip"));

    // Verify critical path styling is present
    assert!(html.contains("critical"));

    // Verify dependencies are rendered
    assert!(html.contains("dep-arrow"));

    // Verify legend
    assert!(html.contains("Critical Path"));
    assert!(html.contains("Milestone"));
}

#[test]
fn render_with_dark_theme() {
    let mut project = Project::new("Dark Theme Test");
    project.start = date(2025, 1, 6);
    project.tasks = vec![
        Task::new("task1").name("Task 1").effort(Duration::days(5)),
        Task::new("task2")
            .name("Task 2")
            .effort(Duration::days(3))
            .depends_on("task1"),
    ];

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).unwrap();

    let renderer = HtmlGanttRenderer::new().dark_theme();
    let html = renderer.render(&project, &schedule).unwrap();

    // Dark theme background color
    assert!(html.contains("#1a1a2e"));
}

#[test]
fn render_with_resource_leveling() {
    let mut project = Project::new("Leveled Project");
    project.start = date(2025, 1, 6);

    project.resources = vec![Resource::new("dev").capacity(1.0)];

    project.tasks = vec![
        Task::new("task1")
            .name("Task 1")
            .effort(Duration::days(5))
            .assign("dev"),
        Task::new("task2")
            .name("Task 2")
            .effort(Duration::days(5))
            .assign("dev"),
        Task::new("task3")
            .name("Task 3")
            .effort(Duration::days(3))
            .depends_on("task1")
            .assign("dev"),
    ];

    // Use solver with leveling
    let solver = CpmSolver::with_leveling();
    let schedule = solver.schedule(&project).unwrap();

    let renderer = HtmlGanttRenderer::new();
    let html = renderer.render(&project, &schedule).unwrap();

    assert!(html.contains("Task 1"));
    assert!(html.contains("Task 2"));
    assert!(html.contains("Task 3"));
}

#[test]
fn render_static_chart() {
    let mut project = Project::new("Static Chart");
    project.start = date(2025, 1, 6);
    project.tasks = vec![Task::new("task1").name("Task 1").effort(Duration::days(5))];

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).unwrap();

    let renderer = HtmlGanttRenderer::new().static_chart();
    let html = renderer.render(&project, &schedule).unwrap();

    // Should have basic structure
    assert!(html.contains("<svg"));

    // Should not have interactive JS
    assert!(!html.contains("const taskData"));
}

#[test]
fn render_without_dependencies() {
    let mut project = Project::new("No Deps");
    project.start = date(2025, 1, 6);
    project.tasks = vec![
        Task::new("task1").name("Task 1").effort(Duration::days(5)),
        Task::new("task2")
            .name("Task 2")
            .effort(Duration::days(3))
            .depends_on("task1"),
    ];

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).unwrap();

    let renderer = HtmlGanttRenderer::new().hide_dependencies();
    let html = renderer.render(&project, &schedule).unwrap();

    // Should not have dependency arrows group in SVG
    assert!(!html.contains(r#"<g class="dependencies">"#));
}

// ============================================================================
// Date interval coverage tests
// ============================================================================

#[test]
fn render_short_project_daily_interval() {
    // Project <= 14 days should show daily interval
    let mut project = Project::new("Short Project");
    project.start = date(2025, 1, 6);
    project.tasks = vec![
        Task::new("task1").name("Quick Task").duration(Duration::days(10)),
    ];

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).unwrap();

    let renderer = HtmlGanttRenderer::new();
    let html = renderer.render(&project, &schedule).unwrap();

    assert!(html.contains("Short Project"));
    assert!(html.contains("<svg"));
}

#[test]
fn render_medium_project_weekly_interval() {
    // Project 15-60 days should show weekly interval
    let mut project = Project::new("Medium Project");
    project.start = date(2025, 1, 6);
    project.tasks = vec![
        Task::new("task1").name("Task 1").duration(Duration::days(20)),
        Task::new("task2").name("Task 2").duration(Duration::days(20)).depends_on("task1"),
    ];

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).unwrap();

    let renderer = HtmlGanttRenderer::new();
    let html = renderer.render(&project, &schedule).unwrap();

    assert!(html.contains("Medium Project"));
}

#[test]
fn render_long_project_biweekly_interval() {
    // Project 61-180 days should show bi-weekly interval
    let mut project = Project::new("Long Project");
    project.start = date(2025, 1, 6);
    project.tasks = vec![
        Task::new("task1").name("Phase 1").duration(Duration::days(50)),
        Task::new("task2").name("Phase 2").duration(Duration::days(50)).depends_on("task1"),
    ];

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).unwrap();

    let renderer = HtmlGanttRenderer::new();
    let html = renderer.render(&project, &schedule).unwrap();

    assert!(html.contains("Long Project"));
}

#[test]
fn render_very_long_project_monthly_interval() {
    // Project > 180 days should show monthly interval
    let mut project = Project::new("Very Long Project");
    project.start = date(2025, 1, 6);
    project.tasks = vec![
        Task::new("task1").name("Quarter 1").duration(Duration::days(60)),
        Task::new("task2").name("Quarter 2").duration(Duration::days(60)).depends_on("task1"),
        Task::new("task3").name("Quarter 3").duration(Duration::days(60)).depends_on("task2"),
        Task::new("task4").name("Quarter 4").duration(Duration::days(60)).depends_on("task3"),
    ];

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).unwrap();

    let renderer = HtmlGanttRenderer::new();
    let html = renderer.render(&project, &schedule).unwrap();

    assert!(html.contains("Very Long Project"));
}

#[test]
fn render_with_progress_tracking() {
    let mut project = Project::new("Progress Test");
    project.start = date(2025, 1, 6);
    project.tasks = vec![
        Task::new("done").name("Completed").duration(Duration::days(5)).complete(100.0),
        Task::new("half").name("Half Done").duration(Duration::days(5)).complete(50.0).depends_on("done"),
        Task::new("started").name("Just Started").duration(Duration::days(5)).complete(10.0).depends_on("half"),
        Task::new("pending").name("Not Started").duration(Duration::days(5)).depends_on("started"),
    ];

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).unwrap();

    let renderer = HtmlGanttRenderer::new();
    let html = renderer.render(&project, &schedule).unwrap();

    assert!(html.contains("Completed"));
    assert!(html.contains("Half Done"));
}
