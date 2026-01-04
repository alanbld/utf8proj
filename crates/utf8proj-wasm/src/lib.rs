//! WebAssembly bindings for utf8proj scheduling engine
//!
//! This crate provides JavaScript-callable functions for parsing project files
//! and generating schedules directly in the browser.

use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

use utf8proj_core::Scheduler;
use utf8proj_parser::parse_project as parse_proj;
use utf8proj_solver::CpmSolver;

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

    // Convert to JSON-friendly structure
    let result = ScheduleResult {
        project: ProjectInfo {
            name: project.name.clone(),
            start: project.start.to_string(),
            end: schedule.project_end.to_string(),
            duration_days: schedule.project_duration.as_days() as i64,
            overall_progress,
        },
        critical_path: schedule.critical_path.clone(),
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

// JSON output structures

#[derive(Serialize, Deserialize)]
struct ScheduleResult {
    project: ProjectInfo,
    critical_path: Vec<String>,
    tasks: Vec<TaskInfo>,
}

#[derive(Serialize, Deserialize)]
struct ProjectInfo {
    name: String,
    start: String,
    end: String,
    duration_days: i64,
    overall_progress: u8,
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
}
