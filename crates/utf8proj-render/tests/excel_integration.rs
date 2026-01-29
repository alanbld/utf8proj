//! Integration tests for Excel rendering

use chrono::NaiveDate;
use rust_decimal_macros::dec;
use utf8proj_core::{Duration, Money, Project, Renderer, Resource, Scheduler, Task};
use utf8proj_render::{ExcelRenderer, ProgressMode};
use utf8proj_solver::CpmSolver;

fn date(year: i32, month: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(year, month, day).unwrap()
}

/// Create a simple linear project for testing (avoids scheduler overflow bug)
fn create_crm_project() -> Project {
    let mut project = Project::new("CRM Migration to Salesforce");
    project.start = date(2026, 2, 1);
    project.currency = "EUR".to_string();

    project.resources = vec![
        Resource::new("pm")
            .name("Maria Rossi")
            .rate(Money::new(dec!(850), "EUR")),
        Resource::new("sa1")
            .name("Luca Bianchi")
            .rate(Money::new(dec!(800), "EUR")),
        Resource::new("dev1")
            .name("Marco Neri")
            .rate(Money::new(dec!(600), "EUR")),
        Resource::new("trainer")
            .name("Paolo Gialli")
            .rate(Money::new(dec!(500), "EUR")),
    ];

    // Simple linear chain to avoid cross-container dependency overflow
    project.tasks = vec![
        Task::new("kickoff")
            .name("Project Kickoff")
            .duration(Duration::days(1))
            .assign("pm"),
        Task::new("requirements")
            .name("Requirements Analysis")
            .duration(Duration::days(8))
            .depends_on("kickoff")
            .assign("sa1"),
        Task::new("design")
            .name("Solution Design")
            .duration(Duration::days(5))
            .depends_on("requirements")
            .assign("sa1"),
        Task::new("development")
            .name("Development")
            .duration(Duration::days(10))
            .depends_on("design")
            .assign("dev1"),
        Task::new("testing")
            .name("Testing")
            .duration(Duration::days(5))
            .depends_on("development")
            .assign("dev1"),
        Task::new("training")
            .name("User Training")
            .duration(Duration::days(3))
            .depends_on("testing")
            .assign("trainer"),
        Task::new("go_live")
            .name("Go-Live")
            .duration(Duration::days(2))
            .depends_on("training")
            .assign("pm"),
        Task::new("complete")
            .name("Project Complete")
            .milestone()
            .depends_on("go_live"),
    ];

    project
}

#[test]
fn render_crm_project_to_excel() {
    let project = create_crm_project();
    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).unwrap();

    let renderer = ExcelRenderer::new()
        .currency("EUR")
        .weeks(20)
        .hours_per_day(8.0);

    let xlsx = renderer.render(&project, &schedule).unwrap();

    // Verify it's a valid XLSX file (starts with PK zip signature)
    assert!(xlsx.len() > 100);
    assert_eq!(&xlsx[0..2], b"PK");

    // Write to file for inspection (uncomment for local testing)
    // std::fs::write("/tmp/crm_schedule.xlsx", &xlsx).unwrap();
}

#[test]
fn render_excel_with_dependencies() {
    let project = create_crm_project();
    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).unwrap();

    let renderer = ExcelRenderer::new().currency("EUR").weeks(20);

    let xlsx = renderer.render(&project, &schedule).unwrap();
    assert!(xlsx.len() > 100);
}

#[test]
fn render_excel_without_dependencies() {
    let project = create_crm_project();
    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).unwrap();

    let renderer = ExcelRenderer::new()
        .currency("EUR")
        .weeks(20)
        .no_dependencies();

    let xlsx = renderer.render(&project, &schedule).unwrap();
    assert!(xlsx.len() > 100);
}

#[test]
fn render_excel_static_values() {
    let project = create_crm_project();
    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).unwrap();

    let renderer = ExcelRenderer::new()
        .currency("EUR")
        .weeks(20)
        .static_values();

    let xlsx = renderer.render(&project, &schedule).unwrap();
    assert!(xlsx.len() > 100);
}

// =============================================================================
// RFC-0018: Progress Mode Tests
// =============================================================================

/// Create a project with progress data for testing
fn create_project_with_progress() -> Project {
    let mut project = Project::new("Progress Test Project");
    project.start = date(2026, 2, 1);
    project.status_date = Some(date(2026, 2, 15));
    project.currency = "EUR".to_string();

    project.resources = vec![Resource::new("dev")
        .name("Developer")
        .rate(Money::new(dec!(600), "EUR"))];

    project.tasks = vec![
        Task::new("design")
            .name("Design")
            .duration(Duration::days(5))
            .assign("dev")
            .complete(100.0)
            .actual_start(date(2026, 2, 3))
            .actual_finish(date(2026, 2, 7)),
        Task::new("impl")
            .name("Implementation")
            .duration(Duration::days(10))
            .depends_on("design")
            .assign("dev")
            .complete(60.0)
            .actual_start(date(2026, 2, 10)),
        Task::new("test")
            .name("Testing")
            .duration(Duration::days(5))
            .depends_on("impl")
            .assign("dev")
            .complete(0.0),
    ];

    project
}

#[test]
fn render_excel_with_progress_mode_none() {
    let project = create_project_with_progress();
    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).unwrap();

    // Progress mode None should produce valid output (no progress columns)
    let renderer = ExcelRenderer::new()
        .currency("EUR")
        .weeks(20)
        .with_progress_mode(ProgressMode::None);

    let xlsx = renderer.render(&project, &schedule).unwrap();
    assert!(xlsx.len() > 100);
    assert_eq!(&xlsx[0..2], b"PK");
}

#[test]
fn render_excel_with_progress_mode_columns() {
    let project = create_project_with_progress();
    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).unwrap();

    // Progress mode Columns should add Complete%, Remaining, Act.Start, Act.End columns
    let renderer = ExcelRenderer::new()
        .currency("EUR")
        .weeks(20)
        .with_progress_mode(ProgressMode::Columns);

    let xlsx = renderer.render(&project, &schedule).unwrap();
    assert!(xlsx.len() > 100);
    assert_eq!(&xlsx[0..2], b"PK");

    // The file should be larger due to progress columns
    // (we can't easily verify column content without parsing the XLSX)
}

#[test]
fn render_excel_with_progress_mode_visual() {
    let project = create_project_with_progress();
    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).unwrap();

    // Progress mode Visual should work
    let renderer = ExcelRenderer::new()
        .currency("EUR")
        .weeks(20)
        .with_progress_mode(ProgressMode::Visual);

    let xlsx = renderer.render(&project, &schedule).unwrap();
    assert!(xlsx.len() > 100);
    assert_eq!(&xlsx[0..2], b"PK");
}

#[test]
fn render_excel_with_progress_mode_full() {
    let project = create_project_with_progress();
    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).unwrap();

    // Progress mode Full should work
    let renderer = ExcelRenderer::new()
        .currency("EUR")
        .weeks(20)
        .with_progress_mode(ProgressMode::Full);

    let xlsx = renderer.render(&project, &schedule).unwrap();
    assert!(xlsx.len() > 100);
    assert_eq!(&xlsx[0..2], b"PK");
}

#[test]
fn render_excel_with_progress_and_status_date() {
    let project = create_project_with_progress();
    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).unwrap();

    // Progress mode with explicit status date
    let renderer = ExcelRenderer::new()
        .currency("EUR")
        .weeks(20)
        .with_progress_mode(ProgressMode::Columns)
        .with_status_date(date(2026, 2, 20));

    let xlsx = renderer.render(&project, &schedule).unwrap();
    assert!(xlsx.len() > 100);
    assert_eq!(&xlsx[0..2], b"PK");
}

#[test]
fn render_excel_progress_without_dependencies() {
    let project = create_project_with_progress();
    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).unwrap();

    // Progress mode should work with no_dependencies too
    let renderer = ExcelRenderer::new()
        .currency("EUR")
        .weeks(20)
        .no_dependencies()
        .with_progress_mode(ProgressMode::Columns);

    let xlsx = renderer.render(&project, &schedule).unwrap();
    assert!(xlsx.len() > 100);
    assert_eq!(&xlsx[0..2], b"PK");
}
