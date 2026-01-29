//! WebAssembly bindings for utf8proj scheduling engine
//!
//! This crate provides JavaScript-callable functions for parsing project files
//! and generating schedules directly in the browser.

use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

use utf8proj_core::{CollectingEmitter, LevelingMode, Renderer, Scheduler, Severity};
use utf8proj_parser::parse_project as parse_proj;
use utf8proj_render::{
    ExcelConfig, ExcelRenderer, HtmlGanttRenderer, MermaidRenderer, NowLineConfig, PlantUmlRenderer,
};
use utf8proj_solver::{
    analyze_project, classify_scheduling_mode, level_resources_with_options, AnalysisConfig,
    CpmSolver, LevelingOptions, LevelingStrategy,
};

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
    fn find_task<'a>(
        tasks: &'a [utf8proj_core::Task],
        id: &str,
    ) -> Option<&'a utf8proj_core::Task> {
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
                    let dur = task
                        .duration
                        .or(task.effort)
                        .unwrap_or(utf8proj_core::Duration::zero())
                        .minutes;
                    let pct = task.effective_percent_complete() as i64;
                    total_weighted += pct * dur;
                    total_duration += dur;
                }
            }
            (total_weighted, total_duration)
        }
        let (weighted, duration) = collect_leaf_progress(tasks);
        if duration == 0 {
            0
        } else {
            (weighted as f64 / duration as f64).round() as u8
        }
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
            status_date: project.status_date.map(|d| d.to_string()),
        },
        critical_path: schedule.critical_path.clone(),
        diagnostics,
        tasks: schedule
            .tasks
            .values()
            .map(|t| {
                let orig_task = find_task(&project.tasks, &t.task_id);
                let (is_milestone, is_container, child_count, derived_progress, dependencies) =
                    match orig_task {
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

                // Get explicit_remaining from original task if set
                let explicit_remaining_days = orig_task
                    .and_then(|task| task.explicit_remaining)
                    .map(|d| d.as_days() as i64);

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
                    explicit_remaining_days,
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
        lines.insert(
            task_end_line,
            format!("    complete: {}%", new_percent as i32),
        );
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

    serde_json::to_string(&info).map_err(|e| JsValue::from_str(&format!("JSON error: {}", e)))
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
        if calendar
            .holidays
            .iter()
            .any(|h| h.start <= current && current <= h.end)
        {
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

// ============================================================================
// Playground Class
// ============================================================================

/// Interactive playground for scheduling projects in the browser
#[wasm_bindgen]
pub struct Playground {
    project: Option<utf8proj_core::Project>,
    schedule: Option<utf8proj_core::Schedule>,
    dark_theme: bool,
    resource_leveling: bool,
    /// RFC-0014 Phase 3: Enable optimal CP solver for small clusters
    optimal_leveling: bool,
    /// RFC-0014 Phase 3: Max cluster size for optimal solver (default: 50)
    optimal_threshold: usize,
    /// RFC-0014 Phase 3: Timeout per cluster in ms (default: 5000)
    optimal_timeout_ms: u64,
    last_error: Option<String>,
    focus_patterns: Vec<String>,
    context_depth: usize,
    /// RFC-0016: Diagnostics from last schedule operation
    diagnostics: Vec<utf8proj_core::Diagnostic>,
    /// RFC-0017: Show now line on Gantt chart (default: true)
    show_now_line: bool,
}

#[wasm_bindgen]
impl Playground {
    /// Create a new playground instance
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            project: None,
            schedule: None,
            dark_theme: false,
            resource_leveling: false,
            optimal_leveling: false,
            optimal_threshold: 50,
            optimal_timeout_ms: 5000,
            last_error: None,
            focus_patterns: vec![],
            context_depth: 1,
            diagnostics: vec![],
            show_now_line: true, // RFC-0017: enabled by default
        }
    }

    /// Parse and schedule a project
    ///
    /// # Arguments
    /// * `input` - The project definition string
    /// * `format` - Either "native" or "tjp"
    ///
    /// # Returns
    /// JSON object with schedule data or error
    pub fn schedule(&mut self, input: &str, format: &str) -> JsValue {
        match self.schedule_internal(input, format) {
            Ok(data) => serde_wasm_bindgen::to_value(&PlaygroundResult {
                success: true,
                error: None,
                data: Some(data),
            })
            .unwrap(),
            Err(e) => {
                self.last_error = Some(e.clone());
                serde_wasm_bindgen::to_value(&PlaygroundResult {
                    success: false,
                    error: Some(e),
                    data: None,
                })
                .unwrap()
            }
        }
    }

    /// Validate input without scheduling
    ///
    /// # Returns
    /// JSON object with validation errors (if any)
    pub fn validate(&self, input: &str, format: &str) -> JsValue {
        let result = match format {
            "tjp" => utf8proj_parser::parse_tjp(input),
            _ => parse_proj(input),
        };

        match result {
            Ok(_) => serde_wasm_bindgen::to_value(&ValidationResult {
                valid: true,
                errors: vec![],
            })
            .unwrap(),
            Err(e) => {
                let error = ValidationError {
                    message: e.to_string(),
                    line: None,
                    column: None,
                };
                serde_wasm_bindgen::to_value(&ValidationResult {
                    valid: false,
                    errors: vec![error],
                })
                .unwrap()
            }
        }
    }

    /// Check if a project is loaded
    pub fn has_project(&self) -> bool {
        self.project.is_some()
    }

    /// Check if a schedule is computed
    pub fn has_schedule(&self) -> bool {
        self.schedule.is_some()
    }

    /// Render the scheduled project as an HTML Gantt chart
    ///
    /// # Returns
    /// HTML string containing the Gantt chart, or empty string if no schedule
    pub fn render_gantt(&self) -> String {
        match (&self.project, &self.schedule) {
            (Some(project), Some(schedule)) => {
                let mut renderer = HtmlGanttRenderer::new();
                if self.dark_theme {
                    renderer = renderer.dark_theme();
                }
                // Apply focus view settings
                if !self.focus_patterns.is_empty() {
                    renderer = renderer.focus(self.focus_patterns.clone());
                    renderer = renderer.context_depth(self.context_depth);
                }
                // RFC-0017: Configure now line
                if self.show_now_line {
                    // Use project.status_date if set, otherwise use today
                    let status_date = project
                        .status_date
                        .unwrap_or_else(|| chrono::Local::now().date_naive());
                    renderer = renderer.with_now_line(NowLineConfig::with_status_date(status_date));
                } else {
                    renderer = renderer.with_now_line(NowLineConfig::disabled());
                }
                renderer.render(project, schedule).unwrap_or_default()
            }
            _ => String::new(),
        }
    }

    /// Render just the SVG portion of the Gantt chart (for embedding)
    pub fn render_gantt_svg(&self) -> String {
        // For now, return full HTML - SVG extraction can be added later
        self.render_gantt()
    }

    /// Render as Mermaid Gantt diagram
    ///
    /// # Returns
    /// Mermaid diagram syntax, or empty string if no schedule
    pub fn render_mermaid(&self) -> String {
        match (&self.project, &self.schedule) {
            (Some(project), Some(schedule)) => {
                let renderer = MermaidRenderer::new();
                renderer.render(project, schedule).unwrap_or_default()
            }
            _ => String::new(),
        }
    }

    /// Render as PlantUML Gantt diagram
    ///
    /// # Returns
    /// PlantUML diagram syntax, or empty string if no schedule
    pub fn render_plantuml(&self) -> String {
        match (&self.project, &self.schedule) {
            (Some(project), Some(schedule)) => {
                let renderer = PlantUmlRenderer::new();
                renderer.render(project, schedule).unwrap_or_default()
            }
            _ => String::new(),
        }
    }

    /// Render as Excel workbook (XLSX)
    ///
    /// # Returns
    /// Raw bytes of the XLSX file as a Vec<u8>, or empty if no schedule
    pub fn render_xlsx(&self) -> Vec<u8> {
        match (&self.project, &self.schedule) {
            (Some(project), Some(schedule)) => {
                let renderer = ExcelRenderer::new();
                renderer.render(project, schedule).unwrap_or_default()
            }
            _ => Vec::new(),
        }
    }

    /// Render as Excel workbook with configuration (RFC-0009)
    ///
    /// # Arguments
    /// * `config` - JSON configuration object (ExcelConfig)
    ///   - `scale`: "daily" or "weekly" (default: "weekly")
    ///   - `currency`: currency symbol (default: "EUR")
    ///   - `auto_fit`: auto-fit timeframe to project (default: true)
    ///   - `weeks`: number of weeks (if auto_fit=false, scale=weekly)
    ///   - `days`: number of days (if auto_fit=false, scale=daily)
    ///   - `hours_per_day`: working hours per day (default: 8.0)
    ///   - `include_summary`: include executive summary sheet (default: true)
    ///   - `show_dependencies`: show dependency columns (default: true)
    ///
    /// # Returns
    /// Raw bytes of the XLSX file as a Vec<u8>, or empty if no schedule
    pub fn render_xlsx_with_config(&self, config: JsValue) -> Vec<u8> {
        match (&self.project, &self.schedule) {
            (Some(project), Some(schedule)) => {
                // Deserialize config from JavaScript, falling back to defaults
                let config: ExcelConfig =
                    serde_wasm_bindgen::from_value(config).unwrap_or_default();

                let renderer = config.to_renderer();
                renderer.render(project, schedule).unwrap_or_default()
            }
            _ => Vec::new(),
        }
    }

    /// Get schedule data as JSON
    pub fn get_schedule_json(&self) -> String {
        match (&self.project, &self.schedule) {
            (Some(_), Some(schedule)) => serde_json::to_string_pretty(schedule).unwrap_or_default(),
            _ => String::new(),
        }
    }

    /// Get the last error message
    pub fn get_last_error(&self) -> Option<String> {
        self.last_error.clone()
    }

    // =========================================================================
    // RFC-0016: Audit Console - Diagnostic Access Methods
    // =========================================================================

    /// Get all diagnostics from the last schedule operation (RFC-0016)
    ///
    /// Returns a JSON array of diagnostic objects with code, severity, and message.
    /// Includes all severity levels (error, warning, hint, info).
    pub fn get_diagnostics(&self) -> String {
        let infos: Vec<DiagnosticInfo> = self
            .diagnostics
            .iter()
            .map(|d| DiagnosticInfo {
                code: format!("{}", d.code),
                severity: d.severity.as_str().to_string(),
                message: d.message.clone(),
            })
            .collect();
        serde_json::to_string(&infos).unwrap_or_else(|_| "[]".to_string())
    }

    /// Get diagnostics filtered by minimum severity (RFC-0016)
    ///
    /// # Arguments
    /// * `min_severity` - Minimum severity: "error", "warning", "hint", or "info"
    ///
    /// Returns diagnostics at or above the specified severity level.
    pub fn get_diagnostics_filtered(&self, min_severity: &str) -> String {
        let min_level = match min_severity.to_lowercase().as_str() {
            "error" => 0,
            "warning" => 1,
            "hint" => 2,
            "info" | _ => 3,
        };

        let infos: Vec<DiagnosticInfo> = self
            .diagnostics
            .iter()
            .filter(|d| {
                let level = match d.severity {
                    Severity::Error => 0,
                    Severity::Warning => 1,
                    Severity::Hint => 2,
                    Severity::Info => 3,
                };
                level <= min_level
            })
            .map(|d| DiagnosticInfo {
                code: format!("{}", d.code),
                severity: d.severity.as_str().to_string(),
                message: d.message.clone(),
            })
            .collect();
        serde_json::to_string(&infos).unwrap_or_else(|_| "[]".to_string())
    }

    /// Get leveling-specific diagnostics only (RFC-0016)
    ///
    /// Returns only L001-L007 diagnostics (leveling audit trail).
    /// Useful for understanding what the leveler changed.
    pub fn get_leveling_audit(&self) -> String {
        let infos: Vec<DiagnosticInfo> = self
            .diagnostics
            .iter()
            .filter(|d| {
                let code = format!("{}", d.code);
                code.starts_with('L')
            })
            .map(|d| DiagnosticInfo {
                code: format!("{}", d.code),
                severity: d.severity.as_str().to_string(),
                message: d.message.clone(),
            })
            .collect();
        serde_json::to_string(&infos).unwrap_or_else(|_| "[]".to_string())
    }

    /// Enable or disable dark theme
    pub fn set_dark_theme(&mut self, enabled: bool) {
        self.dark_theme = enabled;
    }

    /// Enable or disable resource leveling
    pub fn set_resource_leveling(&mut self, enabled: bool) {
        self.resource_leveling = enabled;
    }

    /// Enable or disable optimal CP solver for small clusters (RFC-0014 Phase 3)
    ///
    /// When enabled, clusters with <= optimal_threshold tasks are solved
    /// using constraint programming for optimal makespan. Larger clusters
    /// fall back to heuristic leveling.
    pub fn set_optimal_leveling(&mut self, enabled: bool) {
        self.optimal_leveling = enabled;
    }

    /// Set maximum cluster size for optimal solver (RFC-0014 Phase 3)
    ///
    /// Default is 50 tasks. Clusters larger than this use heuristic leveling.
    pub fn set_optimal_threshold(&mut self, threshold: usize) {
        self.optimal_threshold = threshold;
    }

    /// Set timeout per cluster for optimal solver in milliseconds (RFC-0014 Phase 3)
    ///
    /// Default is 5000ms (5 seconds). If the solver times out, it falls back
    /// to heuristic leveling for that cluster.
    pub fn set_optimal_timeout(&mut self, timeout_ms: u64) {
        self.optimal_timeout_ms = timeout_ms;
    }

    /// Enable or disable now line rendering on Gantt chart (RFC-0017)
    ///
    /// When enabled (default), a vertical red line is drawn at the status date
    /// or today's date on the Gantt chart.
    pub fn set_show_now_line(&mut self, enabled: bool) {
        self.show_now_line = enabled;
    }

    /// Get current now line setting (RFC-0017)
    pub fn get_show_now_line(&self) -> bool {
        self.show_now_line
    }

    /// Set focus patterns for focus view (RFC-0006)
    ///
    /// Patterns can match task IDs or names:
    /// - "6.3.2" matches task ID 6.3.2 and all children
    /// - "*.3.*" uses glob matching
    /// - "Migration" matches tasks with "Migration" in the name
    pub fn set_focus(&mut self, patterns: Vec<String>) {
        self.focus_patterns = patterns;
    }

    /// Set context depth for non-focused tasks
    ///
    /// * `0` = hide all non-focused tasks completely
    /// * `1` = show only top-level non-focused tasks (default)
    /// * `2` = show two levels of non-focused hierarchy
    pub fn set_context_depth(&mut self, depth: usize) {
        self.context_depth = depth;
    }

    /// Clear focus view settings (show all tasks)
    pub fn clear_focus(&mut self) {
        self.focus_patterns.clear();
        self.context_depth = 1;
    }

    /// Get example project in native format
    pub fn get_example_native() -> String {
        EXAMPLE_NATIVE.to_string()
    }

    /// Get example project in TJP format
    pub fn get_example_tjp() -> String {
        EXAMPLE_TJP.to_string()
    }

    /// Get hierarchical project example with nested tasks
    pub fn get_example_hierarchical() -> String {
        EXAMPLE_HIERARCHICAL.to_string()
    }

    /// Get progress tracking example with completion percentages
    pub fn get_example_progress() -> String {
        EXAMPLE_PROGRESS.to_string()
    }

    /// Get focus view example demonstrating filtering large projects (RFC-0006)
    pub fn get_example_focus() -> String {
        EXAMPLE_FOCUS.to_string()
    }

    /// Get temporal regimes example demonstrating work/event/deadline modes (RFC-0012)
    pub fn get_example_temporal_regimes() -> String {
        EXAMPLE_TEMPORAL_REGIMES.to_string()
    }

    /// Get resource leveling example demonstrating conflict resolution (RFC-0003, RFC-0014)
    pub fn get_example_leveling() -> String {
        EXAMPLE_LEVELING.to_string()
    }
}

// Non-WASM methods for internal use and testing
impl Playground {
    /// Internal schedule implementation (RFC-0016: captures diagnostics)
    ///
    /// This method is used by both the WASM binding and unit tests.
    /// Not exposed to WASM because Result<PlaygroundScheduleData, String> is not WASM-compatible.
    #[allow(private_interfaces)]
    pub fn schedule_internal(
        &mut self,
        input: &str,
        format: &str,
    ) -> Result<PlaygroundScheduleData, String> {
        self.last_error = None;
        self.diagnostics.clear();

        // Parse based on format
        let project = match format {
            "tjp" => utf8proj_parser::parse_tjp(input),
            _ => parse_proj(input),
        }
        .map_err(|e| e.to_string())?;

        // Schedule (base schedule first)
        let solver = CpmSolver::new();
        let base_schedule = solver.schedule(&project).map_err(|e| e.to_string())?;

        // Apply resource leveling if enabled (RFC-0003, RFC-0014)
        let schedule = if self.resource_leveling {
            let calendar = project.calendars.first().cloned().unwrap_or_default();

            // Resolve optimal leveling: CLI settings override, then project-level config
            let use_optimal =
                self.optimal_leveling || matches!(project.leveling_mode, LevelingMode::Optimal);
            let threshold = project.optimal_threshold.unwrap_or(self.optimal_threshold);
            let timeout = project
                .optimal_timeout_ms
                .unwrap_or(self.optimal_timeout_ms);

            let options = LevelingOptions {
                // Use CriticalPathFirst in WASM: Hybrid requires rayon (threading)
                // and std::time::Instant which are unavailable in browser WASM runtime
                strategy: LevelingStrategy::CriticalPathFirst,
                max_project_delay_factor: None,
                use_optimal,
                optimal_threshold: threshold,
                optimal_timeout_ms: timeout,
            };

            let leveling_result =
                level_resources_with_options(&project, &base_schedule, &calendar, &options);

            // RFC-0016: Capture leveling diagnostics
            self.diagnostics.extend(leveling_result.diagnostics);

            leveling_result.leveled_schedule
        } else {
            base_schedule
        };

        // RFC-0016: Run analyze_project to get additional diagnostics
        let mut emitter = CollectingEmitter::new();
        let config = AnalysisConfig::default();
        analyze_project(&project, Some(&schedule), &config, &mut emitter);
        self.diagnostics.extend(emitter.diagnostics);

        // Store for rendering
        self.project = Some(project);
        self.schedule = Some(schedule.clone());

        Ok(PlaygroundScheduleData {
            tasks: schedule.tasks.len(),
            duration_days: schedule.project_duration.as_days() as i64,
            critical_path: schedule.critical_path.clone(),
        })
    }
}

impl Default for Playground {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Serialize)]
struct PlaygroundResult {
    success: bool,
    error: Option<String>,
    data: Option<PlaygroundScheduleData>,
}

#[derive(Serialize)]
struct PlaygroundScheduleData {
    tasks: usize,
    duration_days: i64,
    critical_path: Vec<String>,
}

#[derive(Serialize)]
struct ValidationResult {
    valid: bool,
    errors: Vec<ValidationError>,
}

#[derive(Serialize)]
struct ValidationError {
    message: String,
    line: Option<usize>,
    column: Option<usize>,
}

// ============================================================================
// Example Templates
// ============================================================================

const EXAMPLE_NATIVE: &str = r#"# CRM Migration Project
# A comprehensive example showing utf8proj capabilities

project "CRM Migration" {
    start: 2026-01-06
    currency: EUR
}

# Define calendars
calendar "standard" {
    working_days: mon, tue, wed, thu, fri
    working_hours: 09:00-17:00
    holiday "New Year" 2026-01-01
}

# Define resources with rates
resource pm "Project Manager" {
    rate: 120/hour
    capacity: 1.0
}

resource dev "Developer" {
    rate: 100/hour
    capacity: 1.0
}

resource qa "QA Engineer" {
    rate: 85/hour
    capacity: 1.0
}

# Phase 1: Discovery
task discovery "Discovery Phase" {
    task kickoff "Project Kickoff" {
        effort: 1d
        assign: pm
    }
    task requirements "Requirements Analysis" {
        effort: 5d
        depends: kickoff
        assign: pm, dev
    }
    task design "System Design" {
        effort: 3d
        depends: requirements
        assign: dev
    }
}

# Phase 2: Development
task development "Development Phase" {
    task api "Backend API" {
        effort: 10d
        depends: discovery.design
        assign: dev
    }
    task ui "Frontend UI" {
        effort: 8d
        depends: discovery.design
        assign: dev
    }
    task integration "Integration" {
        effort: 5d
        depends: api, ui
        assign: dev
    }
}

# Phase 3: Testing & Deployment
task testing "Testing Phase" {
    task unit "Unit Testing" {
        effort: 3d
        depends: development.integration
        assign: qa
    }
    task uat "User Acceptance Testing" {
        effort: 5d
        depends: unit
        assign: qa, pm
    }
}

milestone golive "Go Live" {
    depends: testing.uat
}
"#;

const EXAMPLE_TJP: &str = r#"# TaskJuggler Format Example

project prj "Software Release" 2026-01-06 - 2026-03-06 {
    timezone "UTC"
    currency "USD"
}

resource dev "Development Team" {
    efficiency 1.0
}

resource qa "QA Team" {
    efficiency 1.0
}

task planning "Planning" {
    effort 3d
    allocate dev
}

task dev_phase "Development" {
    depends !planning

    task backend "Backend Services" {
        effort 10d
        allocate dev
    }

    task frontend "Frontend App" {
        effort 8d
        allocate dev
    }
}

task testing "Testing" {
    depends !dev_phase
    effort 5d
    allocate qa
}

task release "Release" {
    depends !testing
    milestone
}
"#;

const EXAMPLE_HIERARCHICAL: &str = r#"# Hierarchical Task Structure Example
# Demonstrates nested tasks and container derivation

project "Product Launch" {
    start: 2026-02-01
}

resource team "Launch Team" {
    capacity: 1.0
}

# Top-level container with nested phases
task product_launch "Product Launch Program" {

    # Phase 1: Research
    task research "Market Research" {
        task surveys "Customer Surveys" {
            effort: 5d
            assign: team
        }
        task analysis "Competitor Analysis" {
            effort: 3d
            depends: surveys
            assign: team
        }
        task report "Research Report" {
            effort: 2d
            depends: analysis
            assign: team
        }
    }

    # Phase 2: Development
    task development "Product Development" {
        task prototype "Prototype" {
            effort: 10d
            depends: product_launch.research.report
            assign: team
        }
        task testing "Product Testing" {
            effort: 5d
            depends: prototype
            assign: team
        }
        task refinement "Refinement" {
            effort: 3d
            depends: testing
            assign: team
        }
    }

    # Phase 3: Launch
    task launch "Launch Execution" {
        task materials "Marketing Materials" {
            effort: 5d
            depends: product_launch.development.refinement
            assign: team
        }
        task campaign "Launch Campaign" {
            effort: 3d
            depends: materials
            assign: team
        }
    }
}

milestone launch_complete "Product Launched" {
    depends: product_launch.launch.campaign
}
"#;

const EXAMPLE_PROGRESS: &str = r#"# Progress Tracking Example (RFC-0008)
# Demonstrates status_date, completion %, and remaining duration

project "Sprint Progress" {
    start: 2026-01-06
    status_date: 2026-01-20  # Report as of this date
}

resource dev "Developer" {
    capacity: 1.0
}

# Completed task - locked to actual dates
task done "Feature A - Complete" {
    duration: 5d
    complete: 100%
    assign: dev
}

# In-progress task - schedules remaining from status_date
task in_progress "Feature B - In Progress" {
    duration: 10d
    complete: 40%           # 40% done = 6 days remaining
    depends: done
    assign: dev
}

# In-progress with explicit remaining override
task custom_remaining "Feature C - Custom Remaining" {
    duration: 8d
    complete: 50%           # Would be 4d remaining
    remaining: 6d           # But we know it needs 6d more
    depends: in_progress
    assign: dev
}

# Future task - schedules from predecessor
task future "Feature D - Not Started" {
    duration: 5d
    complete: 0%
    depends: custom_remaining
    assign: dev
}

# Milestone tracks overall completion
milestone sprint_end "Sprint Complete" {
    depends: future
}
"#;

const EXAMPLE_FOCUS: &str = r#"# Focus View Example (RFC-0006)
# Try the Focus feature to filter this large project!
#
# How to use Focus View:
# 1. Enter a pattern in the "Focus" field (e.g., "backend" or "api")
# 2. Adjust "Context Depth" to show more/less surrounding structure
# 3. Click "Run" to see filtered Gantt chart
#
# Pattern examples:
#   "backend"     - Shows all tasks containing "backend"
#   "api"         - Shows API-related tasks
#   "stream_a"    - Shows entire Stream A
#   "testing"     - Shows all testing tasks

project "Enterprise Platform" {
    start: 2026-01-06
}

resource dev "Developer" { capacity: 1.0 }
resource qa "QA Engineer" { capacity: 1.0 }
resource devops "DevOps" { capacity: 1.0 }

# Stream A: Backend Services
task stream_a "Stream A: Backend" {
    task backend_api "Backend API Development" {
        task api_design "API Design" {
            effort: 5d
            assign: dev
        }
        task api_impl "API Implementation" {
            effort: 15d
            depends: api_design
            assign: dev
        }
        task api_testing "API Testing" {
            effort: 5d
            depends: api_impl
            assign: qa
        }
    }
    task backend_db "Database Layer" {
        task db_schema "Schema Design" {
            effort: 3d
            assign: dev
        }
        task db_migration "Migration Scripts" {
            effort: 5d
            depends: db_schema
            assign: dev
        }
    }
}

# Stream B: Frontend
task stream_b "Stream B: Frontend" {
    task frontend_ui "UI Components" {
        task ui_design "UI Design" {
            effort: 5d
            assign: dev
        }
        task ui_impl "UI Implementation" {
            effort: 12d
            depends: ui_design
            assign: dev
        }
    }
    task frontend_integration "Frontend Integration" {
        task integration_api "API Integration" {
            effort: 5d
            depends: stream_a.backend_api.api_impl, stream_b.frontend_ui.ui_impl
            assign: dev
        }
        task integration_testing "Integration Testing" {
            effort: 5d
            depends: integration_api
            assign: qa
        }
    }
}

# Stream C: Infrastructure
task stream_c "Stream C: Infrastructure" {
    task infra_setup "Infrastructure Setup" {
        task cloud_setup "Cloud Environment" {
            effort: 3d
            assign: devops
        }
        task ci_cd "CI/CD Pipeline" {
            effort: 5d
            depends: cloud_setup
            assign: devops
        }
    }
    task infra_security "Security Hardening" {
        task security_audit "Security Audit" {
            effort: 3d
            depends: infra_setup.ci_cd
            assign: devops
        }
        task security_fixes "Security Fixes" {
            effort: 5d
            depends: security_audit
            assign: devops
        }
    }
}

# Final Phase: Deployment
task deployment "Deployment Phase" {
    task staging "Staging Deployment" {
        effort: 2d
        depends: stream_b.frontend_integration.integration_testing, stream_c.infra_security.security_fixes
        assign: devops
    }
    task uat "User Acceptance Testing" {
        effort: 5d
        depends: staging
        assign: qa
    }
    task production "Production Deployment" {
        effort: 1d
        depends: uat
        assign: devops
    }
}

milestone launch "Platform Launch" {
    depends: deployment.production
}
"#;

const EXAMPLE_TEMPORAL_REGIMES: &str = r#"# Temporal Regimes Example (RFC-0012)
# Demonstrates Work, Event, and Deadline regimes
#
# Three kinds of time in projects:
# - Work: Effort-bearing tasks (coding, construction)
# - Event: Exact calendar dates (releases, approvals)
# - Deadline: External contractual deadlines

project "Product Release" {
    start: 2026-01-06
}

resource dev "Developer" {
    rate: 100/hour
    capacity: 1.0
}

# Work regime (default) - advances on working days only
task development "Development Sprint" {
    regime: work                    # Can be omitted (default)
    task coding "Feature Coding" {
        effort: 10d
        assign: dev
    }
    task code_review "Code Review" {
        effort: 2d
        depends: coding
        assign: dev
    }
}

# Event regime - exact dates, even on weekends
# Milestones implicitly use Event regime
milestone approval "Stakeholder Approval" {
    regime: event                   # Explicit (milestone implies this)
    depends: development.code_review
    start_no_earlier_than: 2026-01-25  # Saturday? Stays on Saturday!
}

# Events with constraints on specific dates
task release "Release Weekend" {
    regime: event                   # Stays on weekend if scheduled there
    duration: 0d
    depends: approval
    start_no_earlier_than: 2026-02-01  # Sunday release - exact date
}

# Work resumes on Monday after weekend event
task post_release "Post-Release Support" {
    regime: work                    # Starts Monday after Sunday release
    effort: 5d
    depends: release
    assign: dev
}

# Deadline regime - external contractual dates
task contract_deadline "Contract Delivery" {
    regime: deadline               # Must finish by this date
    duration: 0d
    depends: post_release
    finish_no_later_than: 2026-02-15  # Even if it's a holiday
}

milestone delivered "Contract Complete" {
    depends: contract_deadline
}
"#;

const EXAMPLE_LEVELING: &str = r#"# Resource Leveling Example (RFC-0003, RFC-0014)
# Demonstrates resource conflict detection and resolution
#
# How to use:
# 1. First run WITHOUT "Resource Leveling" checked - see overallocation
# 2. Then enable "Resource Leveling" checkbox and run again
# 3. Notice how tasks are delayed to resolve conflicts
#
# For optimal leveling (RFC-0014 Phase 3), you can also use:
#   leveling: optimal
# in the project block to enable constraint programming solver

project "Resource Conflicts Demo" {
    start: 2026-01-05
    # Uncomment to enable optimal CP solver:
    # leveling: optimal
    # optimal_threshold: 50
}

# Single developer - can only work on one task at a time
resource dev "Developer" {
    rate: 100/hour
    capacity: 1.0    # 100% capacity = one task at a time
}

# Single QA engineer
resource qa "QA Engineer" {
    rate: 85/hour
    capacity: 1.0
}

# These three tasks all want the developer at the same time!
# Without leveling: they overlap (overallocation)
# With leveling: they're sequenced automatically

task feature_a "Feature A" {
    effort: 5d
    assign: dev      # Needs developer for 5 days
}

task feature_b "Feature B" {
    effort: 5d
    assign: dev      # Also needs developer - CONFLICT!
}

task feature_c "Feature C" {
    effort: 3d
    assign: dev      # Another conflict!
}

# QA also has conflicts - two testing tasks want QA simultaneously
task test_a "Test Feature A" {
    effort: 3d
    depends: feature_a
    assign: qa
}

task test_b "Test Feature B" {
    effort: 3d
    depends: feature_b
    assign: qa       # CONFLICT with test_a!
}

# Final integration needs both features tested
task integration "Integration Testing" {
    effort: 2d
    depends: test_a, test_b
    assign: qa
}

milestone release "Release Ready" {
    depends: integration
}

# Expected behavior:
# - WITHOUT leveling: All features start Jan 5, tests overlap
# - WITH leveling: Features sequenced (A->B->C), tests sequenced
# - Project duration increases but resources aren't overloaded
"#;

// ============================================================================
// JSON output structures
// ============================================================================

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
    /// Status date for progress reporting (RFC-0008)
    status_date: Option<String>,
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
    /// User-specified remaining duration override (RFC-0008)
    explicit_remaining_days: Option<i64>,
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

    // =========================================================================
    // RFC-0014: Resource Leveling Tests
    // =========================================================================

    #[test]
    fn test_example_leveling_parses() {
        // Verify the leveling example parses correctly
        let code = EXAMPLE_LEVELING;
        let result = schedule(code);
        assert!(
            result.is_ok(),
            "Leveling example should parse: {:?}",
            result.err()
        );
    }

    #[test]
    fn test_playground_resource_leveling_settings() {
        // Test that resource leveling can be toggled
        // Note: We can't test the actual schedule() method in native tests
        // because it returns JsValue which requires wasm-bindgen runtime
        let mut pg = Playground::new();

        // Default should be false
        assert!(!pg.resource_leveling);

        // Enable leveling
        pg.set_resource_leveling(true);
        assert!(pg.resource_leveling);

        // Disable leveling
        pg.set_resource_leveling(false);
        assert!(!pg.resource_leveling);
    }

    #[test]
    fn test_playground_optimal_leveling_settings() {
        // Test that optimal leveling settings can be set
        let mut pg = Playground::new();

        // Default values
        assert!(!pg.optimal_leveling);
        assert_eq!(pg.optimal_threshold, 50);
        assert_eq!(pg.optimal_timeout_ms, 5000);

        // Set values
        pg.set_optimal_leveling(true);
        pg.set_optimal_threshold(100);
        pg.set_optimal_timeout(10000);

        assert!(pg.optimal_leveling);
        assert_eq!(pg.optimal_threshold, 100);
        assert_eq!(pg.optimal_timeout_ms, 10000);
    }

    #[test]
    fn test_project_level_leveling_config() {
        // Test that project-level leveling config is parsed
        let project = r#"
project "Optimal Test" {
    start: 2026-01-05
    leveling: optimal
    optimal_threshold: 80
    optimal_timeout: 8000
}

resource dev "Developer" { capacity: 1.0 }
task a "Task A" { effort: 5d assign: dev }
"#;

        // Parse and verify config is read
        let parsed = utf8proj_parser::parse_project(project).expect("Should parse");
        assert_eq!(parsed.leveling_mode, LevelingMode::Optimal);
        assert_eq!(parsed.optimal_threshold, Some(80));
        assert_eq!(parsed.optimal_timeout_ms, Some(8000));
    }

    #[test]
    fn test_schedule_with_resource_conflicts() {
        // Test scheduling with resource conflicts (without leveling)
        let project = r#"
project "Conflict Test" {
    start: 2026-01-05
}

resource dev "Developer" { capacity: 1.0 }

task a "Task A" { duration: 5d assign: dev }
task b "Task B" { duration: 5d assign: dev }
task c "Task C" { duration: 5d depends: a, b assign: dev }
"#;

        let result = schedule(project).expect("Should schedule");
        // Without leveling, tasks a and b start on same day
        assert!(result.contains("Task A"));
        assert!(result.contains("Task B"));
    }

    #[test]
    fn test_all_examples_parse() {
        // Verify all examples parse without errors
        let examples = [
            ("native", EXAMPLE_NATIVE),
            ("tjp", EXAMPLE_TJP),
            ("hierarchical", EXAMPLE_HIERARCHICAL),
            ("progress", EXAMPLE_PROGRESS),
            ("focus", EXAMPLE_FOCUS),
            ("temporal", EXAMPLE_TEMPORAL_REGIMES),
            ("leveling", EXAMPLE_LEVELING),
        ];

        for (name, code) in examples {
            let result = if name == "tjp" {
                utf8proj_parser::parse_tjp(code)
            } else {
                utf8proj_parser::parse_project(code)
            };
            assert!(
                result.is_ok(),
                "Example '{}' should parse: {:?}",
                name,
                result.err()
            );
        }
    }

    #[test]
    fn test_leveling_example_has_conflicts() {
        // Verify the leveling example actually has resource conflicts
        let parsed = utf8proj_parser::parse_project(EXAMPLE_LEVELING)
            .expect("Should parse leveling example");

        // Should have developer resource
        assert!(parsed.resources.iter().any(|r| r.id == "dev"));

        // Should have multiple tasks assigned to dev
        let dev_task_count = parsed
            .tasks
            .iter()
            .filter(|t| t.assigned.iter().any(|a| a.resource_id == "dev"))
            .count();
        assert!(
            dev_task_count >= 3,
            "Should have at least 3 tasks assigned to dev"
        );
    }

    // =========================================================================
    // RFC-0016: Audit Console Tests (TDD)
    // =========================================================================

    const CONFLICTING_TASKS_PROJECT: &str = r#"
project "Conflict Test" {
    start: 2026-01-05
}

resource dev "Developer" { capacity: 1.0 }

task a "Task A" { effort: 5d assign: dev }
task b "Task B" { effort: 5d assign: dev }
task c "Task C" { effort: 3d depends: b assign: dev }
"#;

    #[test]
    fn test_get_diagnostics_returns_json() {
        // RFC-0016: get_diagnostics() should return valid JSON array
        let mut pg = Playground::new();
        pg.set_resource_leveling(true);
        let _ = pg.schedule_internal(CONFLICTING_TASKS_PROJECT, "native");
        assert!(pg.has_schedule(), "Schedule should succeed");

        let diagnostics = pg.get_diagnostics();
        assert!(!diagnostics.is_empty(), "Diagnostics should not be empty");

        // Should be valid JSON
        let parsed: Result<Vec<DiagnosticInfo>, _> = serde_json::from_str(&diagnostics);
        assert!(
            parsed.is_ok(),
            "Diagnostics should be valid JSON: {}",
            diagnostics
        );
    }

    #[test]
    fn test_playground_captures_leveling_diagnostics() {
        // RFC-0016: Leveling should produce L001 diagnostics
        let mut pg = Playground::new();
        pg.set_resource_leveling(true);
        let _ = pg.schedule_internal(CONFLICTING_TASKS_PROJECT, "native");
        assert!(pg.has_schedule(), "Schedule should succeed");

        let diagnostics = pg.get_diagnostics();
        let parsed: Vec<DiagnosticInfo> =
            serde_json::from_str(&diagnostics).expect("Should parse diagnostics JSON");

        // Should have at least one L001 (overallocation resolved)
        let has_l001 = parsed.iter().any(|d| d.code == "L001");
        assert!(
            has_l001,
            "Should have L001 diagnostic. Got: {:?}",
            parsed.iter().map(|d| &d.code).collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_get_leveling_audit_returns_only_l_codes() {
        // RFC-0016: get_leveling_audit() filters to L001-L007 only
        let mut pg = Playground::new();
        pg.set_resource_leveling(true);
        let _ = pg.schedule_internal(CONFLICTING_TASKS_PROJECT, "native");
        assert!(pg.has_schedule(), "Schedule should succeed");

        let audit = pg.get_leveling_audit();
        let parsed: Vec<DiagnosticInfo> =
            serde_json::from_str(&audit).expect("Should parse audit JSON");

        // All diagnostics should be L-prefixed
        for diag in &parsed {
            assert!(
                diag.code.starts_with('L'),
                "Leveling audit should only contain L codes, got: {}",
                diag.code
            );
        }
    }

    #[test]
    fn test_diagnostics_empty_without_leveling() {
        // RFC-0016: Without leveling, no L001 diagnostics
        let mut pg = Playground::new();
        pg.set_resource_leveling(false); // leveling OFF
        let _ = pg.schedule_internal(CONFLICTING_TASKS_PROJECT, "native");
        assert!(pg.has_schedule(), "Schedule should succeed");

        let audit = pg.get_leveling_audit();
        let parsed: Vec<DiagnosticInfo> =
            serde_json::from_str(&audit).expect("Should parse audit JSON");

        // Should have no L001 (no leveling happened)
        let has_l001 = parsed.iter().any(|d| d.code == "L001");
        assert!(!has_l001, "Should NOT have L001 without leveling");
    }

    #[test]
    fn test_get_diagnostics_filtered_by_severity() {
        // RFC-0016: Filter diagnostics by minimum severity
        let mut pg = Playground::new();
        pg.set_resource_leveling(true);
        let _ = pg.schedule_internal(CONFLICTING_TASKS_PROJECT, "native");
        assert!(pg.has_schedule(), "Schedule should succeed");

        // Filter to warnings and above (should exclude hints and info)
        let warnings = pg.get_diagnostics_filtered("warning");
        let parsed: Vec<DiagnosticInfo> =
            serde_json::from_str(&warnings).expect("Should parse filtered JSON");

        // All should be warning or error severity
        for diag in &parsed {
            assert!(
                diag.severity == "warning" || diag.severity == "error",
                "Filtered diagnostics should be warning+, got: {} ({})",
                diag.severity,
                diag.code
            );
        }
    }

    // =========================================================================
    // RFC-0017: Now Line Tests (TDD)
    // =========================================================================

    #[test]
    fn test_now_line_default_enabled() {
        // RFC-0017: Now line should be enabled by default
        let pg = Playground::new();
        assert!(pg.show_now_line, "Now line should be enabled by default");
        assert!(pg.get_show_now_line(), "get_show_now_line should return true");
    }

    #[test]
    fn test_now_line_can_be_disabled() {
        // RFC-0017: Now line can be toggled
        let mut pg = Playground::new();
        pg.set_show_now_line(false);
        assert!(!pg.show_now_line, "Now line should be disabled");
        assert!(!pg.get_show_now_line(), "get_show_now_line should return false");
    }

    #[test]
    fn test_now_line_rendered_when_enabled() {
        // RFC-0017: When enabled, now line should appear in Gantt
        let mut pg = Playground::new();
        pg.set_show_now_line(true);
        let _ = pg.schedule_internal(CONFLICTING_TASKS_PROJECT, "native");
        assert!(pg.has_schedule(), "Schedule should succeed");

        let html = pg.render_gantt();
        assert!(
            html.contains("now-line"),
            "Gantt should contain now-line element when enabled"
        );
    }

    #[test]
    fn test_now_line_not_rendered_when_disabled() {
        // RFC-0017: When disabled, now line should NOT appear in Gantt
        let mut pg = Playground::new();
        pg.set_show_now_line(false);
        let _ = pg.schedule_internal(CONFLICTING_TASKS_PROJECT, "native");
        assert!(pg.has_schedule(), "Schedule should succeed");

        let html = pg.render_gantt();
        assert!(
            !html.contains("class=\"now-line"),
            "Gantt should NOT contain now-line element when disabled"
        );
    }

    #[test]
    fn test_now_line_uses_status_date() {
        // RFC-0017: Now line should use project.status_date if set
        let project_with_status_date = r#"
project "Status Date Test" {
    start: 2026-01-05
    status_date: 2026-01-10
}

task a "Task A" { duration: 10d }
task b "Task B" { duration: 5d depends: a }
"#;

        let mut pg = Playground::new();
        pg.set_show_now_line(true);
        let _ = pg.schedule_internal(project_with_status_date, "native");
        assert!(pg.has_schedule(), "Schedule should succeed");

        let html = pg.render_gantt();
        assert!(
            html.contains("now-line"),
            "Gantt should contain now-line"
        );
        // The status date should appear in the label
        assert!(
            html.contains("2026-01-10"),
            "Gantt should contain status date label"
        );
    }
}
