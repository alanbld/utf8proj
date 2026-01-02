//! # utf8proj-wasm
//!
//! WebAssembly bindings for the utf8proj scheduling engine.
//!
//! This crate provides a browser-compatible API for:
//! - Parsing TJP and native DSL project files
//! - Scheduling with CPM and resource leveling
//! - Rendering interactive Gantt charts
//!
//! ## Usage from JavaScript
//!
//! ```javascript
//! import init, { Playground } from './utf8proj_wasm.js';
//!
//! await init();
//! const playground = new Playground();
//!
//! const result = playground.schedule(`
//!     project "My Project" { start: 2025-01-06 }
//!     task design "Design" { effort: 5d }
//!     task impl "Implementation" { effort: 10d, depends: design }
//! `, "native");
//!
//! console.log(result);
//! const gantt = playground.render_gantt();
//! document.getElementById('gantt').innerHTML = gantt;
//! ```

use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

use utf8proj_core::{Project, Renderer, Schedule, Scheduler};
use utf8proj_parser::{parse_project, parse_tjp};
use utf8proj_render::HtmlGanttRenderer;
use utf8proj_solver::CpmSolver;

// Initialize panic hook for better error messages
#[wasm_bindgen(start)]
pub fn init() {
    console_error_panic_hook::set_once();
}

/// Main playground interface for the browser
#[wasm_bindgen]
pub struct Playground {
    project: Option<Project>,
    schedule: Option<Schedule>,
    last_error: Option<String>,
    resource_leveling: bool,
    dark_theme: bool,
}

#[wasm_bindgen]
impl Playground {
    /// Create a new playground instance
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            project: None,
            schedule: None,
            last_error: None,
            resource_leveling: false,
            dark_theme: false,
        }
    }

    /// Enable or disable resource leveling
    #[wasm_bindgen]
    pub fn set_resource_leveling(&mut self, enabled: bool) {
        self.resource_leveling = enabled;
    }

    /// Enable or disable dark theme
    #[wasm_bindgen]
    pub fn set_dark_theme(&mut self, enabled: bool) {
        self.dark_theme = enabled;
    }

    /// Parse and schedule a project
    ///
    /// # Arguments
    /// * `input` - The project definition string
    /// * `format` - Either "native" or "tjp"
    ///
    /// # Returns
    /// JSON object with schedule data or error
    #[wasm_bindgen]
    pub fn schedule(&mut self, input: &str, format: &str) -> JsValue {
        self.last_error = None;
        self.project = None;
        self.schedule = None;

        // Parse the input
        let project = match format {
            "native" => match parse_project(input) {
                Ok(p) => p,
                Err(e) => {
                    self.last_error = Some(e.to_string());
                    return serde_wasm_bindgen::to_value(&ScheduleResult {
                        success: false,
                        error: Some(e.to_string()),
                        data: None,
                    })
                    .unwrap_or(JsValue::NULL);
                }
            },
            "tjp" => match parse_tjp(input) {
                Ok(p) => p,
                Err(e) => {
                    self.last_error = Some(e.to_string());
                    return serde_wasm_bindgen::to_value(&ScheduleResult {
                        success: false,
                        error: Some(e.to_string()),
                        data: None,
                    })
                    .unwrap_or(JsValue::NULL);
                }
            },
            _ => {
                let err = format!("Unknown format: {}. Use 'native' or 'tjp'", format);
                self.last_error = Some(err.clone());
                return serde_wasm_bindgen::to_value(&ScheduleResult {
                    success: false,
                    error: Some(err),
                    data: None,
                })
                .unwrap_or(JsValue::NULL);
            }
        };

        // Schedule the project
        let solver = if self.resource_leveling {
            CpmSolver::with_leveling()
        } else {
            CpmSolver::new()
        };

        let schedule = match solver.schedule(&project) {
            Ok(s) => s,
            Err(e) => {
                self.last_error = Some(e.to_string());
                return serde_wasm_bindgen::to_value(&ScheduleResult {
                    success: false,
                    error: Some(e.to_string()),
                    data: None,
                })
                .unwrap_or(JsValue::NULL);
            }
        };

        // Build result data
        let tasks: Vec<TaskData> = schedule
            .tasks
            .iter()
            .map(|(id, task)| TaskData {
                id: id.clone(),
                name: project
                    .get_task(id)
                    .map(|t| t.name.clone())
                    .unwrap_or_else(|| id.clone()),
                start: task.start.to_string(),
                finish: task.finish.to_string(),
                duration_days: task.duration.as_days() as i32,
                is_critical: task.is_critical,
                slack_days: task.slack.as_days() as i32,
            })
            .collect();

        let data = ScheduleData {
            project_name: project.name.clone(),
            project_start: project.start.to_string(),
            project_end: schedule.project_end.to_string(),
            duration_days: schedule.project_duration.as_days() as i32,
            tasks,
            critical_path: schedule.critical_path.clone(),
        };

        self.project = Some(project);
        self.schedule = Some(schedule);

        serde_wasm_bindgen::to_value(&ScheduleResult {
            success: true,
            error: None,
            data: Some(data),
        })
        .unwrap_or(JsValue::NULL)
    }

    /// Render the scheduled project as an HTML Gantt chart
    ///
    /// # Returns
    /// HTML string containing the Gantt chart, or empty string if no schedule
    #[wasm_bindgen]
    pub fn render_gantt(&self) -> String {
        let (Some(project), Some(schedule)) = (&self.project, &self.schedule) else {
            return String::new();
        };

        let renderer = if self.dark_theme {
            HtmlGanttRenderer::new().dark_theme()
        } else {
            HtmlGanttRenderer::new()
        };

        match renderer.render(project, schedule) {
            Ok(html) => html,
            Err(e) => format!("<div class='error'>Render error: {}</div>", e),
        }
    }

    /// Render just the SVG portion of the Gantt chart (for embedding)
    #[wasm_bindgen]
    pub fn render_gantt_svg(&self) -> String {
        let (Some(project), Some(schedule)) = (&self.project, &self.schedule) else {
            return String::new();
        };

        // Use the basic SVG renderer
        let renderer = utf8proj_render::SvgRenderer::new();
        match renderer.render(project, schedule) {
            Ok(svg) => svg,
            Err(e) => format!("<text>Error: {}</text>", e),
        }
    }

    /// Validate input without scheduling
    ///
    /// # Returns
    /// JSON object with validation errors (if any)
    #[wasm_bindgen]
    pub fn validate(&self, input: &str, format: &str) -> JsValue {
        let errors = match format {
            "native" => match parse_project(input) {
                Ok(_) => vec![],
                Err(e) => vec![ValidationError {
                    line: extract_line_number(&e.to_string()),
                    column: 0,
                    message: e.to_string(),
                }],
            },
            "tjp" => match parse_tjp(input) {
                Ok(_) => vec![],
                Err(e) => vec![ValidationError {
                    line: extract_line_number(&e.to_string()),
                    column: 0,
                    message: e.to_string(),
                }],
            },
            _ => vec![ValidationError {
                line: 0,
                column: 0,
                message: format!("Unknown format: {}", format),
            }],
        };

        serde_wasm_bindgen::to_value(&ValidationResult { errors }).unwrap_or(JsValue::NULL)
    }

    /// Get the last error message
    #[wasm_bindgen]
    pub fn get_last_error(&self) -> Option<String> {
        self.last_error.clone()
    }

    /// Check if a project is loaded
    #[wasm_bindgen]
    pub fn has_project(&self) -> bool {
        self.project.is_some()
    }

    /// Check if a schedule is computed
    #[wasm_bindgen]
    pub fn has_schedule(&self) -> bool {
        self.schedule.is_some()
    }

    /// Get schedule data as JSON
    #[wasm_bindgen]
    pub fn get_schedule_json(&self) -> String {
        let Some(schedule) = &self.schedule else {
            return "{}".to_string();
        };

        serde_json::to_string_pretty(schedule).unwrap_or_else(|_| "{}".to_string())
    }

    /// Get example project in native format
    #[wasm_bindgen]
    pub fn get_example_native() -> String {
        EXAMPLE_NATIVE.to_string()
    }

    /// Get example project in TJP format
    #[wasm_bindgen]
    pub fn get_example_tjp() -> String {
        EXAMPLE_TJP.to_string()
    }
}

impl Default for Playground {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Data Types for JavaScript
// ============================================================================

#[derive(Serialize, Deserialize)]
struct ScheduleResult {
    success: bool,
    error: Option<String>,
    data: Option<ScheduleData>,
}

#[derive(Serialize, Deserialize)]
struct ScheduleData {
    project_name: String,
    project_start: String,
    project_end: String,
    duration_days: i32,
    tasks: Vec<TaskData>,
    critical_path: Vec<String>,
}

#[derive(Serialize, Deserialize)]
struct TaskData {
    id: String,
    name: String,
    start: String,
    finish: String,
    duration_days: i32,
    is_critical: bool,
    slack_days: i32,
}

#[derive(Serialize, Deserialize)]
struct ValidationResult {
    errors: Vec<ValidationError>,
}

#[derive(Serialize, Deserialize)]
struct ValidationError {
    line: usize,
    column: usize,
    message: String,
}

// ============================================================================
// Helpers
// ============================================================================

/// Extract line number from error message (if present)
fn extract_line_number(error: &str) -> usize {
    // Try to find "line X" pattern (case insensitive)
    let lower = error.to_lowercase();
    if let Some(pos) = lower.find("line ") {
        let rest = &error[pos + 5..];
        let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
        if !digits.is_empty() {
            if let Ok(line) = digits.parse() {
                return line;
            }
        }
    }
    0
}

// ============================================================================
// Example Projects
// ============================================================================

const EXAMPLE_NATIVE: &str = r#"# Software Development Project
# This is an example project in the native DSL format

project "Software Development" {
    start: 2025-01-06
}

resource dev1 "Alice Developer" {
    capacity: 1.0
}

resource dev2 "Bob Engineer" {
    capacity: 1.0
}

resource qa "QA Team" {
    capacity: 1.0
}

# Phase 1: Planning
task planning "Planning Phase" {
    task requirements "Gather Requirements" {
        effort: 5d
        assign: dev1
    }
    task design "System Design" {
        effort: 8d
        depends: requirements
        assign: dev1
    }
}

# Phase 2: Development
task development "Development Phase" {
    task backend "Backend Development" {
        effort: 15d
        depends: planning.design
        assign: dev1
    }
    task frontend "Frontend Development" {
        effort: 12d
        depends: planning.design
        assign: dev2
    }
    task integration "Integration" {
        effort: 5d
        depends: backend, frontend
        assign: dev1
    }
}

# Phase 3: Testing
task testing "Testing Phase" {
    task unit_tests "Unit Testing" {
        effort: 5d
        depends: development.integration
        assign: qa
    }
    task integration_tests "Integration Testing" {
        effort: 5d
        depends: unit_tests
        assign: qa
    }
}

# Milestone
task release "Release v1.0" {
    milestone: true
    depends: testing.integration_tests
}
"#;

const EXAMPLE_TJP: &str = r#"project software "Software Development" 2025-01-06 - 2025-04-30 {
    timezone "UTC"
}

resource dev1 "Alice Developer"
resource dev2 "Bob Engineer"
resource qa "QA Team"

task planning "Planning Phase" {
    task requirements "Gather Requirements" {
        effort 5d
        allocate dev1
    }
    task design "System Design" {
        effort 8d
        depends !requirements
        allocate dev1
    }
}

task development "Development Phase" {
    task backend "Backend Development" {
        effort 15d
        depends planning.design
        allocate dev1
    }
    task frontend "Frontend Development" {
        effort 12d
        depends planning.design
        allocate dev2
    }
    task integration "Integration" {
        effort 5d
        depends !backend, !frontend
        allocate dev1
    }
}

task testing "Testing Phase" {
    task unit_tests "Unit Testing" {
        effort 5d
        depends development.integration
        allocate qa
    }
    task integration_tests "Integration Testing" {
        effort 5d
        depends !unit_tests
        allocate qa
    }
}

task release "Release v1.0" {
    milestone
    depends testing.integration_tests
}
"#;

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn playground_creation() {
        let playground = Playground::new();
        assert!(!playground.has_project());
        assert!(!playground.has_schedule());
    }

    #[test]
    fn example_getters() {
        let native = Playground::get_example_native();
        let tjp = Playground::get_example_tjp();

        assert!(native.contains("project"));
        assert!(tjp.contains("project"));
    }

    #[test]
    fn parse_example_native() {
        // Test parsing the example directly
        let project = parse_project(EXAMPLE_NATIVE).unwrap();
        assert_eq!(project.name, "Software Development");
        assert!(!project.tasks.is_empty());
    }

    #[test]
    fn parse_example_tjp() {
        // Test parsing the TJP example directly
        let project = parse_tjp(EXAMPLE_TJP).unwrap();
        assert_eq!(project.name, "Software Development");
        assert!(!project.tasks.is_empty());
    }

    #[test]
    fn schedule_and_render() {
        // Test the full pipeline without JsValue
        let project = parse_project(EXAMPLE_NATIVE).unwrap();
        let solver = CpmSolver::new();
        let schedule = solver.schedule(&project).unwrap();

        assert!(!schedule.tasks.is_empty());
        assert!(!schedule.critical_path.is_empty());

        // Test rendering
        let renderer = HtmlGanttRenderer::new();
        let html = renderer.render(&project, &schedule).unwrap();
        assert!(html.contains("<!DOCTYPE html>"));
        assert!(html.contains("Software Development"));
    }

    #[test]
    fn schedule_with_leveling() {
        let project = parse_project(EXAMPLE_NATIVE).unwrap();
        let solver = CpmSolver::with_leveling();
        let schedule = solver.schedule(&project).unwrap();

        assert!(!schedule.tasks.is_empty());
    }

    #[test]
    fn render_dark_theme() {
        let project = parse_project(EXAMPLE_NATIVE).unwrap();
        let solver = CpmSolver::new();
        let schedule = solver.schedule(&project).unwrap();

        let renderer = HtmlGanttRenderer::new().dark_theme();
        let html = renderer.render(&project, &schedule).unwrap();
        assert!(html.contains("#1a1a2e"));
    }

    #[test]
    fn extract_line_number_test() {
        // The function looks for "line " followed by digits
        assert_eq!(extract_line_number("Syntax error at line 5, column 3: expected token"), 5);
        assert_eq!(extract_line_number("Error on line 10"), 10);
        assert_eq!(extract_line_number("No line number here"), 0);
    }
}
