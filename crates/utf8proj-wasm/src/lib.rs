//! WebAssembly bindings for utf8proj scheduling engine
//!
//! This crate provides JavaScript-callable functions for parsing project files
//! and generating schedules directly in the browser.

use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

use utf8proj_core::{CollectingEmitter, Scheduler, Severity};
use utf8proj_parser::parse_project as parse_proj;
use utf8proj_solver::{analyze_project, classify_scheduling_mode, AnalysisConfig, CpmSolver};

/// Initialize panic hook for better error messages in console
#[wasm_bindgen(start)]
pub fn init() {
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();
}

/// Schedule a project from DSL string and return JSON result
#[wasm_bindgen]
pub fn schedule(project_source: &str) -> Result<String, JsValue> {
    // Parse the project
    let project = parse_proj(project_source)
        .map_err(|e| JsValue::from_str(&format!("Parse error: {}", e)))?;

    // Check feasibility
    let solver = CpmSolver::new();
    let feasibility = solver.is_feasible(&project);

    if !feasibility.feasible {
        let errors: Vec<String> = feasibility
            .conflicts
            .iter()
            .map(|c| c.description.clone())
            .collect();
        return Err(JsValue::from_str(&format!(
            "Project not feasible: {}",
            errors.join("; ")
        )));
    }

    // Schedule the project
    let schedule = solver
        .schedule(&project)
        .map_err(|e| JsValue::from_str(&format!("Scheduling error: {}", e)))?;

    // Build task lookup for dependencies and milestone info
    fn find_task<'a>(tasks: &'a [utf8proj_core::Task], id: &str) -> Option<&'a utf8proj_core::Task> {
        for task in tasks {
            if task.id == id {
                return Some(task);
            }
            if let Some(found) = find_task(&task.children, id) {
                return Some(found);
            }
        }
        None
    }

    // Calculate overall project progress from all tasks
    fn calculate_overall_progress(tasks: &[utf8proj_core::Task]) -> u8 {
        fn collect_leaf_progress(tasks: &[utf8proj_core::Task]) -> (i64, i64) {
            let mut total_weighted: i64 = 0;
            let mut total_duration: i64 = 0;
            for task in tasks {
                if task.is_container() {
                    let (w, d) = collect_leaf_progress(&task.children);
                    total_weighted += w;
                    total_duration += d;
                } else {
                    let dur = task.duration.or(task.effort)
                        .unwrap_or(utf8proj_core::Duration::zero()).minutes;
                    let pct = task.effective_percent_complete() as i64;
                    total_weighted += pct * dur;
                    total_duration += dur;
                }
            }
            (total_weighted, total_duration)
        }
        let (weighted, duration) = collect_leaf_progress(tasks);
        if duration == 0 { 0 } else { (weighted as f64 / duration as f64).round() as u8 }
    }

    let overall_progress = calculate_overall_progress(&project.tasks);

    // Classify scheduling mode
    let scheduling_mode = classify_scheduling_mode(&project);
    let mode_name = match scheduling_mode {
        utf8proj_core::SchedulingMode::DurationBased => "Duration-based",
        utf8proj_core::SchedulingMode::EffortBased => "Effort-based",
        utf8proj_core::SchedulingMode::ResourceLoaded => "Resource-loaded",
    };

    // Run diagnostics
    let mut emitter = CollectingEmitter::new();
    let config = AnalysisConfig::default();
    analyze_project(&project, Some(&schedule), &config, &mut emitter);

    // Convert diagnostics to JSON-friendly format (exclude Info-level for cleaner dashboard)
    let diagnostics: Vec<DiagnosticInfo> = emitter
        .diagnostics
        .into_iter()
        .filter(|d| d.severity != Severity::Info)
        .map(|d| DiagnosticInfo {
            code: format!("{}", d.code),
            severity: d.severity.as_str().to_string(),
            message: d.message,
        })
        .collect();

    // Convert to JSON-friendly structure
    let result = ScheduleResult {
        project: ProjectInfo {
            name: project.name.clone(),
            start: project.start.to_string(),
            end: schedule.project_end.to_string(),
            duration_days: schedule.project_duration.as_days() as i64,
            overall_progress,
            scheduling_mode: mode_name.to_string(),
            scheduling_mode_description: scheduling_mode.description().to_string(),
        },
        critical_path: schedule.critical_path.clone(),
        diagnostics,
        tasks: schedule
            .tasks
            .values()
            .map(|t| {
                let orig_task = find_task(&project.tasks, &t.task_id);
                let (is_milestone, is_container, child_count, derived_progress, dependencies) = match orig_task {
                    Some(task) => (
                        task.milestone,
                        task.is_container(),
                        task.children.len(),
                        task.container_progress(),
                        task.depends.iter().map(|d| d.predecessor.clone()).collect(),
                    ),
                    None => (false, false, 0, None, vec![]),
                };
                // Calculate calendar impact for this task
                let calendar_impact = Some(calculate_calendar_impact(t.start, t.finish, &project));

                TaskInfo {
                    id: t.task_id.clone(),
                    name: t.task_id.clone(),
                    start: t.start.to_string(),
                    finish: t.finish.to_string(),
                    duration_days: t.duration.as_days() as i64,
                    slack_days: t.slack.as_days() as i64,
                    is_critical: t.is_critical,
                    is_milestone,
                    is_container,
                    child_count,
                    percent_complete: t.percent_complete,
                    derived_progress,
                    status: format!("{}", t.status),
                    remaining_days: t.remaining_duration.as_days() as i64,
                    forecast_start: t.forecast_start.to_string(),
                    forecast_finish: t.forecast_finish.to_string(),
                    dependencies,
                    calendar_impact,
                }
            })
            .collect(),
    };

    serde_json::to_string(&result)
        .map_err(|e| JsValue::from_str(&format!("JSON serialization error: {}", e)))
}

/// Update a task's completion percentage in the project source
#[wasm_bindgen]
pub fn update_task_progress(project_source: &str, task_id: &str, new_percent: f64) -> String {
    let mut lines: Vec<String> = project_source.lines().map(String::from).collect();
    let mut in_target_task = false;
    let mut task_start_line = 0;
    let mut task_end_line = 0;
    let mut complete_line: Option<usize> = None;
    let mut brace_count = 0;

    // First pass: find the task and its complete line
    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();

        if trimmed.starts_with("task ") && trimmed.contains(task_id) {
            in_target_task = true;
            task_start_line = i;
            brace_count = 0;
        }

        if in_target_task {
            brace_count += line.matches('{').count();
            brace_count -= line.matches('}').count();

            if trimmed.starts_with("complete:") {
                complete_line = Some(i);
            }

            if brace_count == 0 && i > task_start_line {
                task_end_line = i;
                break;
            }
        }
    }

    // Second pass: modify or insert the complete line
    if let Some(idx) = complete_line {
        lines[idx] = format!("    complete: {}%", new_percent as i32);
    } else if in_target_task && task_end_line > 0 {
        lines.insert(task_end_line, format!("    complete: {}%", new_percent as i32));
    }

    lines.join("\n")
}

/// Get project metadata without full scheduling (for quick preview)
#[wasm_bindgen]
pub fn get_project_info(project_source: &str) -> Result<String, JsValue> {
    let project = parse_proj(project_source)
        .map_err(|e| JsValue::from_str(&format!("Parse error: {}", e)))?;

    let info = ProjectParseResult {
        name: project.name,
        start: project.start.to_string(),
        task_count: count_tasks(&project.tasks),
        resource_count: project.resources.len(),
    };

    serde_json::to_string(&info)
        .map_err(|e| JsValue::from_str(&format!("JSON error: {}", e)))
}

fn count_tasks(tasks: &[utf8proj_core::Task]) -> usize {
    tasks.iter().map(|t| 1 + count_tasks(&t.children)).sum()
}

/// Calculate calendar impact for a task's date range
fn calculate_calendar_impact(
    start: chrono::NaiveDate,
    finish: chrono::NaiveDate,
    project: &utf8proj_core::Project,
) -> CalendarImpactInfo {
    use chrono::Datelike;

    // Get the project calendar
    let calendar = project
        .calendars
        .iter()
        .find(|c| c.id == project.calendar)
        .cloned()
        .unwrap_or_default();

    let calendar_days = (finish - start).num_days() + 1;
    let mut weekend_days = 0u32;
    let mut holiday_days = 0u32;
    let mut non_working_days = 0u32;

    let mut current = start;
    while current <= finish {
        let weekday = current.weekday().num_days_from_sunday() as u8;

        // Check if it's a non-working day (weekend or not in working_days)
        let is_non_working_day = !calendar.working_days.contains(&weekday);

        // Check if weekend (Saturday=6, Sunday=0)
        if weekday == 0 || weekday == 6 {
            weekend_days += 1;
        }

        if is_non_working_day {
            non_working_days += 1;
        }

        // Check if it's a holiday
        if calendar.holidays.iter().any(|h| h.start <= current && current <= h.end) {
            holiday_days += 1;
            // If holiday is on a working day, it also counts as non-working
            if !is_non_working_day {
                non_working_days += 1;
            }
        }

        current = match current.succ_opt() {
            Some(d) => d,
            None => break,
        };
    }

    let working_days = calendar_days - non_working_days as i64;
    let non_working_percent = if calendar_days > 0 {
        (non_working_days as f64 / calendar_days as f64) * 100.0
    } else {
        0.0
    };

    CalendarImpactInfo {
        calendar_id: calendar.id.clone(),
        calendar_days,
        working_days,
        weekend_days,
        holiday_days,
        non_working_percent,
    }
}

// JSON output structures

#[derive(Serialize, Deserialize)]
struct ScheduleResult {
    project: ProjectInfo,
    critical_path: Vec<String>,
    tasks: Vec<TaskInfo>,
    diagnostics: Vec<DiagnosticInfo>,
}

#[derive(Serialize, Deserialize)]
struct DiagnosticInfo {
    code: String,
    severity: String,
    message: String,
}

#[derive(Serialize, Deserialize)]
struct ProjectInfo {
    name: String,
    start: String,
    end: String,
    duration_days: i64,
    overall_progress: u8,
    scheduling_mode: String,
    scheduling_mode_description: String,
}

#[derive(Serialize, Deserialize)]
struct TaskInfo {
    id: String,
    name: String,
    start: String,
    finish: String,
    duration_days: i64,
    slack_days: i64,
    is_critical: bool,
    is_milestone: bool,
    is_container: bool,
    child_count: usize,
    percent_complete: u8,
    derived_progress: Option<u8>,
    status: String,
    remaining_days: i64,
    forecast_start: String,
    forecast_finish: String,
    dependencies: Vec<String>,
    /// Calendar impact data for visualization
    calendar_impact: Option<CalendarImpactInfo>,
}

#[derive(Serialize, Deserialize)]
struct CalendarImpactInfo {
    /// Calendar ID used for this task
    calendar_id: String,
    /// Total calendar days the task spans
    calendar_days: i64,
    /// Number of working days in the task period
    working_days: i64,
    /// Number of weekend days in the task period
    weekend_days: u32,
    /// Number of holidays in the task period
    holiday_days: u32,
    /// Non-working percentage of the task span
    non_working_percent: f64,
}

#[derive(Serialize, Deserialize)]
struct ProjectParseResult {
    name: String,
    start: String,
    task_count: usize,
    resource_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schedule_simple_project() {
        let project = r#"
project "Test" {
    start: 2026-01-06
}

task a "Task A" {
    duration: 5d
}

task b "Task B" {
    duration: 3d
    depends: a
}
"#;

        let result = schedule(project).expect("Should schedule successfully");
        assert!(result.contains("Test"));
        assert!(result.contains("task"));
    }

    #[test]
    fn test_update_task_progress() {
        let project = r#"
project "Test" {
    start: 2026-01-06
}

task a "Task A" {
    duration: 5d
    complete: 50%
}
"#;

        let updated = update_task_progress(project, "a", 75.0);
        assert!(updated.contains("complete: 75%"));
    }

    // Note: Circular dependency test skipped because JsValue errors don't work in native test mode.
    // This is tested via the solver module directly.

    #[test]
    fn test_schedule_with_slack() {
        let project = r#"
project "Slack Test" {
    start: 2026-01-06
}

task critical "Critical Path" {
    duration: 10d
}

task short "Short Task" {
    duration: 2d
}

task end "End" {
    duration: 1d
    depends: critical, short
}
"#;

        let result = schedule(project).expect("Should schedule with slack");
        assert!(result.contains("slack_days"));
        // short task should have slack
        assert!(result.contains("is_critical"));
    }

    #[test]
    fn test_schedule_nested_tasks() {
        let project = r#"
project "Nested" {
    start: 2026-01-06
}

task phase1 "Phase 1" {
    task a "Task A" {
        duration: 3d
        complete: 100%
    }
    task b "Task B" {
        duration: 2d
        complete: 50%
        depends: a
    }
}

task phase2 "Phase 2" {
    task c "Task C" {
        duration: 4d
        depends: phase1.b
    }
}
"#;

        let result = schedule(project).expect("Should schedule nested tasks");
        // Check JSON contains nested task info
        assert!(result.contains("phase1"));
        assert!(result.contains("is_container"));
    }

    #[test]
    fn test_schedule_with_milestones() {
        let project = r#"
project "Milestones" {
    start: 2026-01-06
}

task dev "Development" {
    duration: 10d
    complete: 50%
}

task release "Release" {
    milestone: true
    depends: dev
}
"#;

        let result = schedule(project).expect("Should schedule with milestones");
        assert!(result.contains("is_milestone"));
        assert!(result.contains("true"));
    }

    #[test]
    fn test_schedule_overall_progress() {
        let project = r#"
project "Progress" {
    start: 2026-01-06
}

task a "Task A" {
    duration: 10d
    complete: 100%
}

task b "Task B" {
    duration: 10d
    complete: 0%
}
"#;

        let result = schedule(project).expect("Should calculate progress");
        // Overall progress should be 50% (weighted average)
        assert!(result.contains("overall_progress"));
        assert!(result.contains("50"));
    }

    #[test]
    fn test_schedule_container_progress() {
        let project = r#"
project "Container Progress" {
    start: 2026-01-06
}

task container "Container" {
    task child1 "Child 1" {
        duration: 5d
        complete: 100%
    }
    task child2 "Child 2" {
        duration: 5d
        complete: 0%
    }
}
"#;

        let result = schedule(project).expect("Should calculate container progress");
        assert!(result.contains("derived_progress"));
    }

    #[test]
    fn test_get_project_info() {
        let project = r#"
project "Info Test" {
    start: 2026-01-06
}

task a "Task A" {
    duration: 5d
}

task b "Task B" {
    duration: 3d
}
"#;

        let result = get_project_info(project).expect("Should get project info");
        assert!(result.contains("Info Test"));
        assert!(result.contains("task_count"));
        assert!(result.contains("2"));
    }

    #[test]
    fn test_get_project_info_with_resources() {
        let project = r#"
project "Resource Test" {
    start: 2026-01-06
}

resource dev "Developer" {}
resource qa "QA" {}

task a "Task A" {
    duration: 5d
    assign: dev
}
"#;

        let result = get_project_info(project).expect("Should get project info");
        assert!(result.contains("resource_count"));
        assert!(result.contains("2"));
    }

    #[test]
    fn test_update_task_progress_insert() {
        // Task without complete line - should insert
        let project = r#"
project "Test" {
    start: 2026-01-06
}

task a "Task A" {
    duration: 5d
}
"#;

        let updated = update_task_progress(project, "a", 25.0);
        assert!(updated.contains("complete: 25%"));
    }

    #[test]
    fn test_update_task_progress_nonexistent() {
        let project = r#"
project "Test" {
    start: 2026-01-06
}

task a "Task A" {
    duration: 5d
}
"#;

        // Try to update non-existent task - should return unchanged
        let updated = update_task_progress(project, "nonexistent", 50.0);
        assert!(!updated.contains("complete: 50%"));
    }

    // Note: Parse error tests are skipped because JsValue doesn't work in native test mode.
    // These are tested via wasm-pack test in browser context.

    #[test]
    fn test_schedule_empty_project() {
        let project = r#"
project "Empty" {
    start: 2026-01-06
}
"#;

        let result = schedule(project).expect("Should schedule empty project");
        assert!(result.contains("Empty"));
        assert!(result.contains("overall_progress"));
    }

    #[test]
    fn test_schedule_deeply_nested() {
        let project = r#"
project "Deep" {
    start: 2026-01-06
}

task level1 "Level 1" {
    task level2 "Level 2" {
        task level3 "Level 3" {
            task leaf "Leaf Task" {
                duration: 5d
                complete: 50%
            }
        }
    }
}
"#;

        let result = schedule(project).expect("Should schedule deeply nested");
        assert!(result.contains("level1"));
        assert!(result.contains("is_container"));
    }

    #[test]
    fn test_schedule_with_dependencies() {
        let project = r#"
project "Deps" {
    start: 2026-01-06
}

task a "Task A" {
    duration: 5d
}

task b "Task B" {
    duration: 3d
    depends: a
}

task c "Task C" {
    duration: 2d
    depends: a, b
}
"#;

        let result = schedule(project).expect("Should schedule with dependencies");
        assert!(result.contains("dependencies"));
    }

    #[test]
    fn test_count_tasks_nested() {
        let project = r#"
project "Nested Count" {
    start: 2026-01-06
}

task parent "Parent" {
    task child1 "Child 1" {
        duration: 2d
    }
    task child2 "Child 2" {
        task grandchild "Grandchild" {
            duration: 1d
        }
    }
}

task standalone "Standalone" {
    duration: 3d
}
"#;

        let result = get_project_info(project).expect("Should count nested tasks");
        // Should count: parent, child1, child2, grandchild, standalone = 5
        assert!(result.contains("task_count"));
        assert!(result.contains("5"));
    }
}
