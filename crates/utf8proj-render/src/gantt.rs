//! Interactive HTML Gantt Chart Renderer
//!
//! Generates standalone HTML files with embedded SVG Gantt charts.
//! Features:
//! - Task bars with critical path highlighting
//! - Dependency arrows (FS, SS, FF, SF)
//! - Hover tooltips with task details
//! - Click to highlight dependencies
//! - Collapsible hierarchical tasks
//! - Responsive zoom controls

use chrono::NaiveDate;
use std::collections::HashMap;
use utf8proj_core::{Project, RenderError, Renderer, Schedule, ScheduledTask, Task};

/// HTML Gantt chart renderer configuration
#[derive(Clone, Debug)]
pub struct HtmlGanttRenderer {
    /// Width of the chart area (excluding labels) in pixels
    pub chart_width: u32,
    /// Height per task row in pixels
    pub row_height: u32,
    /// Width of the label column in pixels
    pub label_width: u32,
    /// Header height in pixels
    pub header_height: u32,
    /// Padding around the chart
    pub padding: u32,
    /// Theme (light or dark)
    pub theme: GanttTheme,
    /// Show dependency arrows
    pub show_dependencies: bool,
    /// Enable interactivity (tooltips, click handlers)
    pub interactive: bool,
    /// Focus view configuration (None = show all tasks)
    pub focus: Option<FocusConfig>,
    /// Now line configuration (RFC-0017)
    pub now_line: NowLineConfig,
    /// Highlight critical path tasks in red (default: true)
    pub highlight_critical: bool,
}

/// Configuration for now line rendering (RFC-0017)
#[derive(Clone, Debug, Default)]
pub struct NowLineConfig {
    /// The resolved status date (from --as-of, project.status_date, or today)
    pub status_date: Option<NaiveDate>,
    /// Whether to show the today line separately (when different from status_date)
    pub show_today: bool,
    /// Disable now line rendering entirely
    pub disabled: bool,
}

impl NowLineConfig {
    /// Create config with status date line enabled
    pub fn with_status_date(date: NaiveDate) -> Self {
        Self {
            status_date: Some(date),
            show_today: false,
            disabled: false,
        }
    }

    /// Also show today line when it differs from status_date
    pub fn with_today(mut self) -> Self {
        self.show_today = true;
        self
    }

    /// Disable all now line rendering
    pub fn disabled() -> Self {
        Self {
            status_date: None,
            show_today: false,
            disabled: true,
        }
    }
}

/// Configuration for focus view rendering
#[derive(Clone, Debug, Default)]
pub struct FocusConfig {
    /// Patterns to match for focused (expanded) tasks
    /// Supports prefix matching (e.g., "6.3.2" matches "6.3.2.1", "6.3.2.2")
    pub focus_patterns: Vec<String>,
    /// Depth to show for non-focused tasks (0 = hide, 1 = top-level only)
    pub context_depth: usize,
}

/// Visibility state for a task in focus view
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskVisibility {
    /// Show task and all descendants expanded
    Expanded,
    /// Show task as collapsed summary bar (children hidden)
    Collapsed,
    /// Do not show task at all
    Hidden,
}

impl FocusConfig {
    /// Create a new focus configuration
    pub fn new(patterns: Vec<String>, context_depth: usize) -> Self {
        Self {
            focus_patterns: patterns,
            context_depth,
        }
    }

    /// Check if a task ID matches any focus pattern
    pub fn matches_focus(&self, task_id: &str, task_name: &str) -> bool {
        if self.focus_patterns.is_empty() {
            return true; // No focus = everything focused
        }
        for pattern in &self.focus_patterns {
            // Check if task_id starts with pattern (prefix match)
            if task_id.starts_with(pattern) || task_id == pattern {
                return true;
            }
            // Check if task_name contains pattern (for WBS codes in names)
            if task_name.contains(pattern) {
                return true;
            }
            // Check pattern as glob (simple * wildcard support)
            if self.glob_match(pattern, task_id) {
                return true;
            }
        }
        false
    }

    /// Simple glob matching with * wildcard
    fn glob_match(&self, pattern: &str, text: &str) -> bool {
        if !pattern.contains('*') {
            return pattern == text;
        }
        let parts: Vec<&str> = pattern.split('*').collect();
        if parts.is_empty() {
            return true;
        }
        let mut pos = 0;
        for (i, part) in parts.iter().enumerate() {
            if part.is_empty() {
                continue;
            }
            if let Some(found) = text[pos..].find(part) {
                if i == 0 && found != 0 {
                    // First part must match at start (unless pattern starts with *)
                    return false;
                }
                pos += found + part.len();
            } else {
                return false;
            }
        }
        // If pattern ends with *, any suffix is OK; otherwise must match to end
        parts
            .last()
            .map_or(true, |p| p.is_empty() || pos == text.len())
    }

    /// Determine visibility of a task based on focus configuration
    pub fn get_visibility(
        &self,
        task_id: &str,
        task_name: &str,
        depth: usize,
        is_ancestor_of_focused: bool,
        is_descendant_of_focused: bool,
    ) -> TaskVisibility {
        // Direct match - always expanded
        if self.matches_focus(task_id, task_name) {
            return TaskVisibility::Expanded;
        }

        // Ancestor of focused task - expanded to show path
        if is_ancestor_of_focused {
            return TaskVisibility::Expanded;
        }

        // Descendant of focused task - expanded
        if is_descendant_of_focused {
            return TaskVisibility::Expanded;
        }

        // Non-focused: check context depth
        if depth < self.context_depth {
            return TaskVisibility::Collapsed;
        }

        TaskVisibility::Hidden
    }
}

/// Color theme for the Gantt chart
#[derive(Clone, Debug)]
pub struct GanttTheme {
    pub critical_color: String,
    pub normal_color: String,
    pub milestone_color: String,
    pub container_color: String,
    pub background_color: String,
    pub grid_color: String,
    pub text_color: String,
    pub header_bg: String,
    pub arrow_color: String,
    pub highlight_color: String,
}

impl Default for GanttTheme {
    fn default() -> Self {
        Self::light()
    }
}

impl GanttTheme {
    pub fn light() -> Self {
        Self {
            critical_color: "#e74c3c".into(),
            normal_color: "#3498db".into(),
            milestone_color: "#9b59b6".into(),
            container_color: "#95a5a6".into(),
            background_color: "#ffffff".into(),
            grid_color: "#ecf0f1".into(),
            text_color: "#2c3e50".into(),
            header_bg: "#f8f9fa".into(),
            arrow_color: "#7f8c8d".into(),
            highlight_color: "#f39c12".into(),
        }
    }

    pub fn dark() -> Self {
        Self {
            critical_color: "#e74c3c".into(),
            normal_color: "#3498db".into(),
            milestone_color: "#9b59b6".into(),
            container_color: "#7f8c8d".into(),
            background_color: "#1a1a2e".into(),
            grid_color: "#2d2d44".into(),
            text_color: "#eaeaea".into(),
            header_bg: "#16213e".into(),
            arrow_color: "#95a5a6".into(),
            highlight_color: "#f39c12".into(),
        }
    }
}

impl Default for HtmlGanttRenderer {
    fn default() -> Self {
        Self {
            chart_width: 900,
            row_height: 32,
            label_width: 450, // Wider to fit WBS codes + task names
            header_height: 60,
            padding: 20,
            theme: GanttTheme::default(),
            show_dependencies: true,
            interactive: true,
            focus: None,
            now_line: NowLineConfig::default(),
            highlight_critical: true,
        }
    }
}

impl HtmlGanttRenderer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Use dark theme
    pub fn dark_theme(mut self) -> Self {
        self.theme = GanttTheme::dark();
        self
    }

    /// Configure chart width
    pub fn chart_width(mut self, width: u32) -> Self {
        self.chart_width = width;
        self
    }

    /// Configure row height
    pub fn row_height(mut self, height: u32) -> Self {
        self.row_height = height;
        self
    }

    /// Disable dependency arrows
    pub fn hide_dependencies(mut self) -> Self {
        self.show_dependencies = false;
        self
    }

    /// Disable interactivity
    pub fn static_chart(mut self) -> Self {
        self.interactive = false;
        self
    }

    /// Disable critical path highlighting (all tasks use normal color)
    pub fn hide_critical_path(mut self) -> Self {
        self.highlight_critical = false;
        self
    }

    /// Set focus view configuration
    ///
    /// Focus view expands tasks matching the patterns while collapsing others.
    ///
    /// # Arguments
    /// * `patterns` - Task ID prefixes or glob patterns to expand
    ///
    /// # Example
    /// ```ignore
    /// let renderer = HtmlGanttRenderer::new()
    ///     .focus(vec!["6.3.2".into(), "8.6".into()]);
    /// ```
    pub fn focus(mut self, patterns: Vec<String>) -> Self {
        let context_depth = self.focus.as_ref().map(|f| f.context_depth).unwrap_or(1);
        self.focus = Some(FocusConfig::new(patterns, context_depth));
        self
    }

    /// Set context depth for non-focused tasks
    ///
    /// * `0` = hide all non-focused tasks
    /// * `1` = show only top-level containers (default)
    /// * `2` = show two levels of hierarchy
    ///
    /// # Example
    /// ```ignore
    /// let renderer = HtmlGanttRenderer::new()
    ///     .focus(vec!["6.3.2".into()])
    ///     .context_depth(0); // Hide all context
    /// ```
    pub fn context_depth(mut self, depth: usize) -> Self {
        if let Some(ref mut focus) = self.focus {
            focus.context_depth = depth;
        } else {
            self.focus = Some(FocusConfig::new(vec![], depth));
        }
        self
    }

    /// Configure now line rendering (RFC-0017)
    ///
    /// The now line is a vertical marker showing the status date on the Gantt chart.
    ///
    /// # Example
    /// ```ignore
    /// use chrono::NaiveDate;
    /// let status_date = NaiveDate::from_ymd_opt(2026, 1, 20).unwrap();
    /// let renderer = HtmlGanttRenderer::new()
    ///     .with_now_line(NowLineConfig::with_status_date(status_date));
    /// ```
    pub fn with_now_line(mut self, config: NowLineConfig) -> Self {
        self.now_line = config;
        self
    }

    /// Compute tight date range from rendered task bars
    ///
    /// Instead of using `project.start` to `schedule.project_end`, this iterates over
    /// the visible tasks and returns their actual min start / max finish with 1-day padding.
    /// Falls back to project metadata if no tasks have schedule data.
    fn compute_visible_date_range(
        &self,
        project: &Project,
        schedule: &Schedule,
        tasks: &[TaskDisplay],
    ) -> (NaiveDate, NaiveDate) {
        let mut min_start: Option<NaiveDate> = None;
        let mut max_finish: Option<NaiveDate> = None;

        for td in tasks {
            if let Some(scheduled) = td.scheduled {
                min_start = Some(match min_start {
                    Some(current) => current.min(scheduled.start),
                    None => scheduled.start,
                });
                max_finish = Some(match max_finish {
                    Some(current) => current.max(scheduled.finish),
                    None => scheduled.finish,
                });
            }
        }

        match (min_start, max_finish) {
            (Some(start), Some(end)) => {
                // 1-day padding on each side
                let padded_start = start - chrono::Duration::days(1);
                let padded_end = end + chrono::Duration::days(1);
                (padded_start, padded_end)
            }
            _ => (project.start, schedule.project_end),
        }
    }

    /// Calculate pixels per day based on date range
    fn pixels_per_day(&self, start: NaiveDate, end: NaiveDate) -> f64 {
        let days = (end - start).num_days().max(1) as f64;
        self.chart_width as f64 / days
    }

    /// Convert a date to x position
    fn date_to_x(&self, date: NaiveDate, project_start: NaiveDate, px_per_day: f64) -> f64 {
        let days = (date - project_start).num_days() as f64;
        self.padding as f64 + self.label_width as f64 + (days * px_per_day)
    }

    /// Build flat list of tasks with hierarchy info, respecting focus configuration
    fn flatten_tasks_for_display<'a>(
        &self,
        project: &'a Project,
        schedule: &'a Schedule,
    ) -> Vec<TaskDisplay<'a>> {
        let mut all_tasks = Vec::new();
        self.collect_tasks(&project.tasks, schedule, "", 0, &mut all_tasks);

        // If no focus config, return all tasks as expanded
        let Some(ref focus) = self.focus else {
            return all_tasks;
        };

        // First pass: identify which task IDs are directly focused
        let focused_ids: std::collections::HashSet<String> = all_tasks
            .iter()
            .filter(|t| focus.matches_focus(&t.qualified_id, &t.task.name))
            .map(|t| t.qualified_id.clone())
            .collect();

        // Second pass: identify ancestors of focused tasks
        let ancestor_ids: std::collections::HashSet<String> = all_tasks
            .iter()
            .filter(|t| {
                // Check if any focused task starts with this task's ID + "."
                focused_ids.iter().any(|fid| {
                    fid.starts_with(&t.qualified_id)
                        && fid.len() > t.qualified_id.len()
                        && fid.chars().nth(t.qualified_id.len()) == Some('.')
                })
            })
            .map(|t| t.qualified_id.clone())
            .collect();

        // Third pass: compute visibility and filter
        let mut result = Vec::new();
        let mut skip_children_of: Option<String> = None;

        for mut task_display in all_tasks {
            // Skip children of collapsed containers
            if let Some(ref skip_prefix) = skip_children_of {
                if task_display.qualified_id.starts_with(skip_prefix)
                    && task_display.qualified_id.len() > skip_prefix.len()
                {
                    continue;
                }
                skip_children_of = None;
            }

            let _is_focused = focused_ids.contains(&task_display.qualified_id);
            let is_ancestor = ancestor_ids.contains(&task_display.qualified_id);
            let is_descendant = focused_ids.iter().any(|fid| {
                task_display.qualified_id.starts_with(fid)
                    && task_display.qualified_id.len() > fid.len()
            });

            let visibility = focus.get_visibility(
                &task_display.qualified_id,
                &task_display.task.name,
                task_display.depth,
                is_ancestor,
                is_descendant,
            );

            match visibility {
                TaskVisibility::Hidden => continue,
                TaskVisibility::Collapsed => {
                    // Show this task but skip its children
                    if task_display.is_container {
                        skip_children_of = Some(task_display.qualified_id.clone() + ".");
                    }
                    task_display.visibility = TaskVisibility::Collapsed;
                    result.push(task_display);
                }
                TaskVisibility::Expanded => {
                    task_display.visibility = TaskVisibility::Expanded;
                    result.push(task_display);
                }
            }
        }

        result
    }

    #[allow(unknown_lints)]
    #[allow(clippy::self_only_used_in_recursion)]
    fn collect_tasks<'a>(
        &self,
        tasks: &'a [Task],
        schedule: &'a Schedule,
        prefix: &str,
        depth: usize,
        result: &mut Vec<TaskDisplay<'a>>,
    ) {
        for task in tasks {
            let qualified_id = if prefix.is_empty() {
                task.id.clone()
            } else {
                format!("{}.{}", prefix, task.id)
            };

            let scheduled = schedule.tasks.get(&qualified_id);
            let is_container = !task.children.is_empty();

            result.push(TaskDisplay {
                task,
                qualified_id: qualified_id.clone(),
                scheduled,
                depth,
                is_container,
                child_count: task.children.len(),
                visibility: TaskVisibility::Expanded, // Default, may be changed by focus logic
            });

            if !task.children.is_empty() {
                self.collect_tasks(&task.children, schedule, &qualified_id, depth + 1, result);
            }
        }
    }

    /// Generate the complete HTML document
    fn generate_html(
        &self,
        project: &Project,
        schedule: &Schedule,
        tasks: &[TaskDisplay],
    ) -> String {
        let (project_start, project_end) = self.compute_visible_date_range(project, schedule, tasks);
        let px_per_day = self.pixels_per_day(project_start, project_end);

        let total_width = self.padding * 2 + self.label_width + self.chart_width;
        let total_height =
            self.padding * 2 + self.header_height + (tasks.len() as u32 * self.row_height) + 50;

        let svg_content = self.generate_svg(project, schedule, tasks, px_per_day);
        let css = self.generate_css();
        let js = if self.interactive {
            self.generate_js(tasks)
        } else {
            String::new()
        };

        format!(
            r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{title} - Gantt Chart</title>
    <style>
{css}
    </style>
</head>
<body>
    <div class="gantt-container">
        <div class="gantt-header">
            <h1>{title}</h1>
            <div class="gantt-controls">
                <button onclick="zoomIn()" title="Zoom In">+</button>
                <button onclick="zoomOut()" title="Zoom Out">−</button>
                <button onclick="resetZoom()" title="Reset">Reset</button>
            </div>
        </div>
        <div class="gantt-wrapper" id="gantt-wrapper">
            <svg id="gantt-svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}">
{svg_content}
            </svg>
        </div>
        <div class="gantt-legend">
            <span class="legend-item"><span class="legend-box critical"></span>Critical Path</span>
            <span class="legend-item"><span class="legend-box normal"></span>Normal Task</span>
            <span class="legend-item"><span class="legend-diamond"></span>Milestone</span>
            <span class="legend-item"><span class="legend-box container"></span>Container</span>
        </div>
        <div id="tooltip" class="tooltip"></div>
    </div>
    <script>
{js}
    </script>
</body>
</html>"#,
            title = html_escape(&project.name),
            css = css,
            width = total_width,
            height = total_height,
            svg_content = svg_content,
            js = js,
        )
    }

    /// Generate the SVG content (without the outer <svg> tag)
    fn generate_svg(
        &self,
        project: &Project,
        schedule: &Schedule,
        tasks: &[TaskDisplay],
        px_per_day: f64,
    ) -> String {
        let mut svg = String::new();
        let project_start = project.start;
        let project_end = schedule.project_end;

        // Background
        svg.push_str(&format!(
            r#"                <rect width="100%" height="100%" fill="{}"/>"#,
            self.theme.background_color
        ));
        svg.push('\n');

        // Grid
        svg.push_str(&self.render_grid(tasks.len(), project_start, project_end, px_per_day));

        // Header
        svg.push_str(&self.render_header(project_start, project_end, px_per_day));

        // Task bars
        for (row, task_display) in tasks.iter().enumerate() {
            svg.push_str(&self.render_task_row(task_display, row, project_start, px_per_day));
        }

        // Dependency arrows
        if self.show_dependencies {
            svg.push_str(&self.render_dependencies(
                project,
                schedule,
                tasks,
                project_start,
                px_per_day,
            ));
        }

        // Now line (RFC-0017) - rendered on top of everything
        svg.push_str(&self.render_now_line(project_start, project_end, tasks.len(), px_per_day));

        svg
    }

    /// Render the timeline header
    fn render_header(
        &self,
        project_start: NaiveDate,
        project_end: NaiveDate,
        px_per_day: f64,
    ) -> String {
        let mut svg = String::new();

        // Header background
        svg.push_str(&format!(
            r#"                <rect x="{}" y="{}" width="{}" height="{}" fill="{}"/>"#,
            self.padding,
            self.padding,
            self.label_width + self.chart_width,
            self.header_height,
            self.theme.header_bg
        ));
        svg.push('\n');

        // Calculate date interval
        let total_days = (project_end - project_start).num_days();
        let interval_days = if total_days <= 14 {
            1
        } else if total_days <= 60 {
            7
        } else if total_days <= 180 {
            14
        } else {
            30
        };

        // Date labels
        let mut current = project_start;
        while current <= project_end {
            let x = self.date_to_x(current, project_start, px_per_day);

            // Tick mark
            svg.push_str(&format!(
                r#"                <line x1="{x}" y1="{y1}" x2="{x}" y2="{y2}" stroke="{color}" stroke-width="1"/>"#,
                x = x,
                y1 = self.padding + self.header_height - 10,
                y2 = self.padding + self.header_height,
                color = self.theme.text_color
            ));
            svg.push('\n');

            // Date label
            let label = if interval_days == 1 {
                current.format("%d").to_string()
            } else {
                current.format("%b %d").to_string()
            };

            svg.push_str(&format!(
                r#"                <text x="{x}" y="{y}" font-size="11" fill="{color}" text-anchor="middle">{label}</text>"#,
                x = x,
                y = self.padding + self.header_height - 15,
                color = self.theme.text_color,
                label = label
            ));
            svg.push('\n');

            current += chrono::Duration::days(interval_days);
        }

        // Add final label for project end if not already labeled
        // (ensures the end date is always visible on the timeline)
        let last_labeled = current - chrono::Duration::days(interval_days);
        let days_from_last = (project_end - last_labeled).num_days();
        // Add end label if it's at least 3 days from last label (avoid overlap)
        if project_end > last_labeled && days_from_last >= 3 {
            let x = self.date_to_x(project_end, project_start, px_per_day);
            svg.push_str(&format!(
                r#"                <line x1="{x}" y1="{y1}" x2="{x}" y2="{y2}" stroke="{color}" stroke-width="1"/>"#,
                x = x,
                y1 = self.padding + self.header_height - 10,
                y2 = self.padding + self.header_height,
                color = self.theme.text_color
            ));
            svg.push('\n');
            let label = project_end.format("%b %d").to_string();
            svg.push_str(&format!(
                r#"                <text x="{x}" y="{y}" font-size="11" fill="{color}" text-anchor="middle">{label}</text>"#,
                x = x,
                y = self.padding + self.header_height - 15,
                color = self.theme.text_color,
                label = label
            ));
            svg.push('\n');
        }

        // Month/year label
        let month_label = project_start.format("%B %Y").to_string();
        svg.push_str(&format!(
            r#"                <text x="{x}" y="{y}" font-size="14" font-weight="bold" fill="{color}" text-anchor="middle">{label}</text>"#,
            x = self.padding + self.label_width + self.chart_width / 2,
            y = self.padding + 22,
            color = self.theme.text_color,
            label = month_label
        ));
        svg.push('\n');

        svg
    }

    /// Render grid lines
    fn render_grid(
        &self,
        task_count: usize,
        project_start: NaiveDate,
        project_end: NaiveDate,
        px_per_day: f64,
    ) -> String {
        let mut svg = String::new();
        let chart_top = self.padding + self.header_height;
        let chart_bottom = chart_top + (task_count as u32 * self.row_height);

        // Horizontal lines
        for i in 0..=task_count {
            let y = chart_top + (i as u32 * self.row_height);
            svg.push_str(&format!(
                r#"                <line x1="{x1}" y1="{y}" x2="{x2}" y2="{y}" stroke="{color}" stroke-width="1"/>"#,
                x1 = self.padding,
                y = y,
                x2 = self.padding + self.label_width + self.chart_width,
                color = self.theme.grid_color
            ));
            svg.push('\n');
        }

        // Vertical lines
        let total_days = (project_end - project_start).num_days();
        let interval = if total_days <= 30 { 1 } else { 7 };

        let mut current = project_start;
        while current <= project_end {
            let x = self.date_to_x(current, project_start, px_per_day);
            svg.push_str(&format!(
                r#"                <line x1="{x}" y1="{y1}" x2="{x}" y2="{y2}" stroke="{color}" stroke-width="1"/>"#,
                x = x,
                y1 = chart_top,
                y2 = chart_bottom,
                color = self.theme.grid_color
            ));
            svg.push('\n');
            current += chrono::Duration::days(interval);
        }

        svg
    }

    /// Render a single task row
    fn render_task_row(
        &self,
        task_display: &TaskDisplay,
        row: usize,
        project_start: NaiveDate,
        px_per_day: f64,
    ) -> String {
        let mut svg = String::new();

        let y = self.padding + self.header_height + (row as u32 * self.row_height);
        let bar_height = (self.row_height as f64 * 0.6) as u32;
        let bar_y = y + (self.row_height - bar_height) / 2;

        // Check if task is collapsed (focus view)
        let is_collapsed = task_display.visibility == TaskVisibility::Collapsed;

        // Indent for hierarchy
        let indent = task_display.depth as u32 * 16;
        let label_x = self.padding + 8 + indent;

        // Container expand/collapse icon
        // ▶ for collapsed, ▼ for expanded
        if task_display.is_container || is_collapsed {
            let icon_x = label_x - 12;
            let icon_y = y + self.row_height / 2;
            let icon = if is_collapsed { "▶" } else { "▼" };
            let icon_color = if is_collapsed {
                "#9ca3af" // Muted gray for collapsed
            } else {
                &self.theme.text_color
            };
            svg.push_str(&format!(
                r#"                <text x="{x}" y="{y}" font-size="10" fill="{color}" class="collapse-icon" data-task="{id}" style="cursor:pointer">{icon}</text>"#,
                x = icon_x,
                y = icon_y + 4,
                color = icon_color,
                id = task_display.qualified_id,
                icon = icon
            ));
            svg.push('\n');
        }

        // Task label - calculate max chars based on available width
        // ~7px per char at 12px font, subtract indent (16px per level) and some margin
        let available_px = self.label_width.saturating_sub(indent as u32 + 20);
        let max_chars = (available_px / 7) as usize;
        let label = truncate(&task_display.task.name, max_chars.max(10));
        let label_color = if is_collapsed {
            "#9ca3af" // Muted gray for collapsed tasks
        } else {
            &self.theme.text_color
        };
        svg.push_str(&format!(
            r#"                <text x="{x}" y="{y}" font-size="12" fill="{color}">{label}</text>"#,
            x = label_x,
            y = y + self.row_height / 2 + 4,
            color = label_color,
            label = html_escape(&label)
        ));
        svg.push('\n');

        // Task bar (if scheduled)
        if let Some(scheduled) = task_display.scheduled {
            let x_start = self.date_to_x(scheduled.start, project_start, px_per_day);
            let x_end = self.date_to_x(scheduled.finish, project_start, px_per_day);
            let bar_width = (x_end - x_start).max(4.0);

            let is_milestone = scheduled.duration.minutes == 0;

            if is_milestone && !is_collapsed {
                // Diamond for milestone (not shown when collapsed)
                let cx = x_start;
                let cy = (bar_y + bar_height / 2) as f64;
                let size = (bar_height as f64) / 2.0;

                svg.push_str(&format!(
                    r#"                <polygon points="{p1},{p2} {p3},{p4} {p5},{p6} {p7},{p8}" fill="{color}" class="task-bar milestone" data-task="{id}"/>"#,
                    p1 = cx, p2 = cy - size,
                    p3 = cx + size, p4 = cy,
                    p5 = cx, p6 = cy + size,
                    p7 = cx - size, p8 = cy,
                    color = self.theme.milestone_color,
                    id = task_display.qualified_id
                ));
                svg.push('\n');
            } else if is_collapsed {
                // Collapsed container: solid muted bar with distinct style
                let collapsed_color = "#b8c0cc"; // Light gray for collapsed
                let collapsed_bar_height = (bar_height as f64 * 0.7) as u32;
                let collapsed_bar_y = y + (self.row_height - collapsed_bar_height) / 2;

                svg.push_str(&format!(
                    r#"                <rect x="{x}" y="{y}" width="{w}" height="{h}" rx="2" fill="{color}" opacity="0.7" class="task-bar collapsed" data-task="{id}"/>"#,
                    x = x_start,
                    y = collapsed_bar_y,
                    w = bar_width,
                    h = collapsed_bar_height,
                    color = collapsed_color,
                    id = task_display.qualified_id
                ));
                svg.push('\n');
            } else if task_display.is_container {
                // Expanded container bar (bracket style)
                let bracket_height = 6.0;
                svg.push_str(&format!(
                    r#"                <path d="M{x1},{y1} L{x1},{y2} L{x2},{y2} L{x2},{y1}" fill="none" stroke="{color}" stroke-width="3" class="task-bar container" data-task="{id}"/>"#,
                    x1 = x_start,
                    y1 = bar_y as f64 + bracket_height,
                    y2 = bar_y as f64,
                    x2 = x_start + bar_width,
                    color = self.theme.container_color,
                    id = task_display.qualified_id
                ));
                svg.push('\n');
            } else {
                // Regular task bar
                let color = if self.highlight_critical && scheduled.is_critical {
                    &self.theme.critical_color
                } else {
                    &self.theme.normal_color
                };

                svg.push_str(&format!(
                    r#"                <rect x="{x}" y="{y}" width="{w}" height="{h}" rx="3" fill="{color}" class="task-bar" data-task="{id}"/>"#,
                    x = x_start,
                    y = bar_y,
                    w = bar_width,
                    h = bar_height,
                    color = color,
                    id = task_display.qualified_id
                ));
                svg.push('\n');

                // Progress overlay if complete percentage is set
                if let Some(complete) = task_display.task.complete {
                    let progress_width = bar_width * (complete as f64 / 100.0);
                    svg.push_str(&format!(
                        r#"                <rect x="{x}" y="{y}" width="{w}" height="{h}" rx="3" fill="rgba(255,255,255,0.3)"/>"#,
                        x = x_start,
                        y = bar_y,
                        w = progress_width,
                        h = bar_height
                    ));
                    svg.push('\n');
                }
            }
        }

        svg
    }

    /// Render dependency arrows
    fn render_dependencies(
        &self,
        _project: &Project,
        _schedule: &Schedule,
        tasks: &[TaskDisplay],
        project_start: NaiveDate,
        px_per_day: f64,
    ) -> String {
        let mut svg = String::new();
        svg.push_str(r#"                <g class="dependencies">"#);
        svg.push('\n');

        // Build task position map
        let mut task_positions: HashMap<&str, (usize, &ScheduledTask)> = HashMap::new();
        for (row, task_display) in tasks.iter().enumerate() {
            if let Some(scheduled) = task_display.scheduled {
                task_positions.insert(&task_display.qualified_id, (row, scheduled));
            }
        }

        // Draw arrows
        for task_display in tasks {
            if let Some(to_scheduled) = task_display.scheduled {
                for dep in &task_display.task.depends {
                    // Try to find the predecessor
                    let pred_id = self.resolve_dependency(
                        &dep.predecessor,
                        &task_display.qualified_id,
                        &task_positions,
                    );

                    if let Some((from_row, from_scheduled)) =
                        pred_id.and_then(|id| task_positions.get(id.as_str()))
                    {
                        let to_row = task_positions
                            .get(task_display.qualified_id.as_str())
                            .map(|(r, _)| *r)
                            .unwrap_or(0);

                        let arrow = self.render_arrow(
                            *from_row,
                            from_scheduled,
                            to_row,
                            to_scheduled,
                            &dep.dep_type,
                            project_start,
                            px_per_day,
                        );
                        svg.push_str(&arrow);
                    }
                }
            }
        }

        svg.push_str("                </g>\n");
        svg
    }

    /// Resolve dependency path
    fn resolve_dependency(
        &self,
        dep_path: &str,
        from_id: &str,
        positions: &HashMap<&str, (usize, &ScheduledTask)>,
    ) -> Option<String> {
        // Try absolute path first
        if positions.contains_key(dep_path) {
            return Some(dep_path.to_string());
        }

        // Try relative (sibling) resolution
        if let Some(dot_pos) = from_id.rfind('.') {
            let parent = &from_id[..dot_pos];
            let qualified = format!("{}.{}", parent, dep_path);
            if positions.contains_key(qualified.as_str()) {
                return Some(qualified);
            }
        }

        None
    }

    /// Render a dependency arrow
    fn render_arrow(
        &self,
        from_row: usize,
        from_task: &ScheduledTask,
        to_row: usize,
        to_task: &ScheduledTask,
        dep_type: &utf8proj_core::DependencyType,
        project_start: NaiveDate,
        px_per_day: f64,
    ) -> String {
        let bar_height = (self.row_height as f64 * 0.6) as f64;
        let bar_y_offset = (self.row_height as f64 - bar_height) / 2.0;

        let from_y = self.padding as f64
            + self.header_height as f64
            + (from_row as f64 * self.row_height as f64)
            + bar_y_offset
            + bar_height / 2.0;
        let to_y = self.padding as f64
            + self.header_height as f64
            + (to_row as f64 * self.row_height as f64)
            + bar_y_offset
            + bar_height / 2.0;

        let (from_x, to_x) = match dep_type {
            utf8proj_core::DependencyType::FinishToStart => {
                let fx = self.date_to_x(from_task.finish, project_start, px_per_day) + 2.0;
                let tx = self.date_to_x(to_task.start, project_start, px_per_day) - 2.0;
                (fx, tx)
            }
            utf8proj_core::DependencyType::StartToStart => {
                let fx = self.date_to_x(from_task.start, project_start, px_per_day) - 2.0;
                let tx = self.date_to_x(to_task.start, project_start, px_per_day) - 2.0;
                (fx, tx)
            }
            utf8proj_core::DependencyType::FinishToFinish => {
                let fx = self.date_to_x(from_task.finish, project_start, px_per_day) + 2.0;
                let tx = self.date_to_x(to_task.finish, project_start, px_per_day) + 2.0;
                (fx, tx)
            }
            utf8proj_core::DependencyType::StartToFinish => {
                let fx = self.date_to_x(from_task.start, project_start, px_per_day) - 2.0;
                let tx = self.date_to_x(to_task.finish, project_start, px_per_day) + 2.0;
                (fx, tx)
            }
        };

        // Create curved path
        let mid_x = (from_x + to_x) / 2.0;
        let path = if (to_row as i32 - from_row as i32).abs() <= 1 {
            // Simple curve for adjacent rows
            format!(
                "M{},{} C{},{} {},{} {},{}",
                from_x, from_y, mid_x, from_y, mid_x, to_y, to_x, to_y
            )
        } else {
            // More complex path for distant rows
            let offset = 15.0;
            format!(
                "M{},{} L{},{} L{},{} L{},{}",
                from_x,
                from_y,
                from_x + offset,
                from_y,
                from_x + offset,
                to_y,
                to_x,
                to_y
            )
        };

        format!(
            r#"                    <path d="{path}" fill="none" stroke="{color}" stroke-width="1.5" marker-end="url(#arrowhead)" class="dep-arrow"/>
"#,
            path = path,
            color = self.theme.arrow_color
        )
    }

    /// Render the now line (RFC-0017)
    ///
    /// A vertical line indicating the status date on the Gantt chart.
    fn render_now_line(
        &self,
        project_start: NaiveDate,
        project_end: NaiveDate,
        task_count: usize,
        px_per_day: f64,
    ) -> String {
        // Skip if disabled or no status date
        if self.now_line.disabled || self.now_line.status_date.is_none() {
            return String::new();
        }

        let mut svg = String::new();
        let chart_height = self.header_height as f64 + (task_count as f64 * self.row_height as f64);

        // Status date line (primary)
        if let Some(status_date) = self.now_line.status_date {
            // Check if date is within chart range
            if status_date >= project_start && status_date <= project_end {
                let x = self.date_to_x(status_date, project_start, px_per_day);
                let y_start = self.padding as f64;
                let y_end = self.padding as f64 + chart_height;

                // Vertical line
                svg.push_str(&format!(
                    r#"                <line class="now-line status-date" x1="{x}" y1="{y1}" x2="{x}" y2="{y2}" />
"#,
                    x = x,
                    y1 = y_start,
                    y2 = y_end
                ));

                // Label at top
                svg.push_str(&format!(
                    r#"                <text class="now-line-label" x="{x}" y="{y}" text-anchor="middle">{date}</text>
"#,
                    x = x,
                    y = y_start - 5.0,
                    date = status_date
                ));
            }
        }

        // Today line (secondary) - only if show_today is true and differs from status_date
        if self.now_line.show_today {
            let today = chrono::Local::now().date_naive();
            if self.now_line.status_date != Some(today)
                && today >= project_start
                && today <= project_end
            {
                let x = self.date_to_x(today, project_start, px_per_day);
                let y_start = self.padding as f64;
                let y_end = self.padding as f64 + chart_height;

                svg.push_str(&format!(
                    r#"                <line class="now-line today" x1="{x}" y1="{y1}" x2="{x}" y2="{y2}" />
"#,
                    x = x,
                    y1 = y_start,
                    y2 = y_end
                ));
            }
        }

        svg
    }

    /// Generate CSS styles
    fn generate_css(&self) -> String {
        format!(
            r#"        :root {{
            --critical-color: {critical};
            --normal-color: {normal};
            --milestone-color: {milestone};
            --container-color: {container};
            --bg-color: {bg};
            --text-color: {text};
            --highlight-color: {highlight};
        }}
        * {{ margin: 0; padding: 0; box-sizing: border-box; }}
        body {{
            font-family: system-ui, -apple-system, sans-serif;
            background: var(--bg-color);
            color: var(--text-color);
            padding: 20px;
        }}
        .gantt-container {{
            max-width: 100%;
            overflow-x: auto;
        }}
        .gantt-header {{
            display: flex;
            justify-content: space-between;
            align-items: center;
            margin-bottom: 16px;
        }}
        .gantt-header h1 {{
            font-size: 1.5rem;
            font-weight: 600;
        }}
        .gantt-controls button {{
            padding: 8px 16px;
            margin-left: 8px;
            border: 1px solid var(--text-color);
            background: transparent;
            color: var(--text-color);
            cursor: pointer;
            border-radius: 4px;
            font-size: 14px;
        }}
        .gantt-controls button:hover {{
            background: rgba(128,128,128,0.2);
        }}
        .gantt-wrapper {{
            overflow-x: auto;
            border: 1px solid rgba(128,128,128,0.3);
            border-radius: 8px;
        }}
        .gantt-legend {{
            display: flex;
            gap: 24px;
            margin-top: 16px;
            font-size: 13px;
        }}
        .legend-item {{
            display: flex;
            align-items: center;
            gap: 6px;
        }}
        .legend-box {{
            width: 16px;
            height: 12px;
            border-radius: 2px;
        }}
        .legend-box.critical {{ background: var(--critical-color); }}
        .legend-box.normal {{ background: var(--normal-color); }}
        .legend-box.container {{ background: var(--container-color); }}
        .legend-diamond {{
            width: 10px;
            height: 10px;
            background: var(--milestone-color);
            transform: rotate(45deg);
        }}
        .task-bar {{
            cursor: pointer;
            transition: opacity 0.2s;
        }}
        .task-bar:hover {{
            opacity: 0.8;
        }}
        .task-bar.highlighted {{
            stroke: var(--highlight-color);
            stroke-width: 3;
        }}
        .dep-arrow {{
            opacity: 0.6;
            transition: opacity 0.2s;
        }}
        .dep-arrow.highlighted {{
            opacity: 1;
            stroke: var(--highlight-color);
            stroke-width: 2;
        }}
        .tooltip {{
            position: fixed;
            background: rgba(0,0,0,0.9);
            color: white;
            padding: 12px;
            border-radius: 6px;
            font-size: 13px;
            pointer-events: none;
            opacity: 0;
            transition: opacity 0.2s;
            z-index: 1000;
            max-width: 300px;
        }}
        .tooltip.visible {{
            opacity: 1;
        }}
        .tooltip .task-name {{
            font-weight: 600;
            margin-bottom: 8px;
        }}
        .tooltip .task-dates {{
            color: #aaa;
        }}
        /* Now line styles (RFC-0017) */
        .now-line {{
            stroke-width: 2px;
            pointer-events: none;
        }}
        .now-line.status-date {{
            stroke: #E53935;
        }}
        .now-line.today {{
            stroke: #43A047;
            stroke-dasharray: 6,4;
        }}
        .now-line-label {{
            font-size: 10px;
            fill: #E53935;
        }}"#,
            critical = if self.highlight_critical { &self.theme.critical_color } else { &self.theme.normal_color },
            normal = self.theme.normal_color,
            milestone = self.theme.milestone_color,
            container = self.theme.container_color,
            bg = self.theme.background_color,
            text = self.theme.text_color,
            highlight = self.theme.highlight_color,
        )
    }

    /// Generate JavaScript for interactivity
    fn generate_js(&self, tasks: &[TaskDisplay]) -> String {
        // Build task data for JS
        let mut task_data = String::from("const taskData = {\n");
        for task_display in tasks {
            if let Some(scheduled) = task_display.scheduled {
                task_data.push_str(&format!(
                    r#"            "{}": {{ name: "{}", start: "{}", finish: "{}", duration: "{} days", critical: {}, deps: [{}] }},
"#,
                    task_display.qualified_id,
                    html_escape(&task_display.task.name),
                    scheduled.start,
                    scheduled.finish,
                    scheduled.duration.as_days() as i64,
                    scheduled.is_critical,
                    task_display.task.depends.iter()
                        .map(|d| format!("\"{}\"", d.predecessor))
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }
        }
        task_data.push_str("        };\n");

        format!(
            r#"        {task_data}

        // Zoom functionality
        let currentZoom = 1;
        const wrapper = document.getElementById('gantt-wrapper');
        const svg = document.getElementById('gantt-svg');

        function zoomIn() {{
            currentZoom = Math.min(currentZoom * 1.2, 3);
            applyZoom();
        }}

        function zoomOut() {{
            currentZoom = Math.max(currentZoom / 1.2, 0.5);
            applyZoom();
        }}

        function resetZoom() {{
            currentZoom = 1;
            applyZoom();
        }}

        function applyZoom() {{
            svg.style.transform = `scale(${{currentZoom}})`;
            svg.style.transformOrigin = 'top left';
        }}

        // Tooltip functionality
        const tooltip = document.getElementById('tooltip');

        document.querySelectorAll('.task-bar').forEach(bar => {{
            bar.addEventListener('mouseenter', (e) => {{
                const taskId = bar.getAttribute('data-task');
                const data = taskData[taskId];
                if (data) {{
                    tooltip.innerHTML = `
                        <div class="task-name">${{data.name}}</div>
                        <div class="task-dates">${{data.start}} → ${{data.finish}}</div>
                        <div>Duration: ${{data.duration}}</div>
                        ${{data.critical ? '<div style="color:#e74c3c">Critical Path</div>' : ''}}
                    `;
                    tooltip.classList.add('visible');
                }}
            }});

            bar.addEventListener('mousemove', (e) => {{
                tooltip.style.left = (e.clientX + 15) + 'px';
                tooltip.style.top = (e.clientY + 15) + 'px';
            }});

            bar.addEventListener('mouseleave', () => {{
                tooltip.classList.remove('visible');
            }});

            // Click to highlight dependencies
            bar.addEventListener('click', () => {{
                const taskId = bar.getAttribute('data-task');
                highlightDependencies(taskId);
            }});
        }});

        function highlightDependencies(taskId) {{
            // Clear previous highlights
            document.querySelectorAll('.highlighted').forEach(el => {{
                el.classList.remove('highlighted');
            }});

            // Highlight selected task
            const taskBar = document.querySelector(`[data-task="${{taskId}}"]`);
            if (taskBar) taskBar.classList.add('highlighted');

            // Highlight dependencies (simplified - would need full dep graph)
            const data = taskData[taskId];
            if (data && data.deps) {{
                data.deps.forEach(depId => {{
                    const depBar = document.querySelector(`[data-task="${{depId}}"]`);
                    if (depBar) depBar.classList.add('highlighted');
                }});
            }}
        }}

        // Arrow marker definition
        const defs = document.createElementNS('http://www.w3.org/2000/svg', 'defs');
        defs.innerHTML = `
            <marker id="arrowhead" markerWidth="10" markerHeight="7" refX="9" refY="3.5" orient="auto">
                <polygon points="0 0, 10 3.5, 0 7" fill="{arrow_color}" />
            </marker>
        `;
        svg.insertBefore(defs, svg.firstChild);"#,
            task_data = task_data,
            arrow_color = self.theme.arrow_color
        )
    }
}

/// Task display info for rendering
struct TaskDisplay<'a> {
    task: &'a Task,
    qualified_id: String,
    scheduled: Option<&'a ScheduledTask>,
    depth: usize,
    is_container: bool,
    #[allow(dead_code)]
    child_count: usize,
    /// Visibility state for focus view
    visibility: TaskVisibility,
}

impl Renderer for HtmlGanttRenderer {
    type Output = String;

    fn render(&self, project: &Project, schedule: &Schedule) -> Result<String, RenderError> {
        let tasks = self.flatten_tasks_for_display(project, schedule);

        if tasks.is_empty() {
            return Err(RenderError::InvalidData("No tasks to render".into()));
        }

        Ok(self.generate_html(project, schedule, &tasks))
    }
}

/// HTML-escape a string
fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Truncate a string with ellipsis
fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        format!(
            "{}…",
            s.chars().take(max.saturating_sub(1)).collect::<String>()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use std::collections::HashMap;
    use utf8proj_core::{Duration, Schedule, ScheduledTask, TaskStatus};

    fn create_test_project() -> Project {
        let mut project = Project::new("Test Project");
        project.start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        project.tasks.push(
            Task::new("design")
                .name("Design Phase")
                .effort(Duration::days(5)),
        );
        project.tasks.push(
            Task::new("implement")
                .name("Implementation")
                .effort(Duration::days(10))
                .depends_on("design"),
        );
        project.tasks.push(
            Task::new("test")
                .name("Testing")
                .effort(Duration::days(3))
                .depends_on("implement"),
        );
        project
    }

    fn create_test_schedule() -> Schedule {
        let mut tasks = HashMap::new();

        let start1 = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        let finish1 = NaiveDate::from_ymd_opt(2025, 1, 10).unwrap();
        tasks.insert(
            "design".to_string(),
            ScheduledTask {
                task_id: "design".to_string(),
                start: start1,
                finish: finish1,
                duration: Duration::days(5),
                assignments: vec![],
                slack: Duration::zero(),
                is_critical: true,
                early_start: start1,
                early_finish: finish1,
                late_start: start1,
                late_finish: finish1,
                forecast_start: start1,
                forecast_finish: finish1,
                remaining_duration: Duration::days(5),
                percent_complete: 0,
                status: TaskStatus::NotStarted,
                cost_range: None,
                has_abstract_assignments: false,
                baseline_start: start1,
                baseline_finish: finish1,
                start_variance_days: 0,
                finish_variance_days: 0,
            },
        );

        let start2 = NaiveDate::from_ymd_opt(2025, 1, 13).unwrap();
        let finish2 = NaiveDate::from_ymd_opt(2025, 1, 24).unwrap();
        tasks.insert(
            "implement".to_string(),
            ScheduledTask {
                task_id: "implement".to_string(),
                start: start2,
                finish: finish2,
                duration: Duration::days(10),
                assignments: vec![],
                slack: Duration::zero(),
                is_critical: true,
                early_start: start2,
                early_finish: finish2,
                late_start: start2,
                late_finish: finish2,
                forecast_start: start2,
                forecast_finish: finish2,
                remaining_duration: Duration::days(10),
                percent_complete: 0,
                status: TaskStatus::NotStarted,
                cost_range: None,
                has_abstract_assignments: false,
                baseline_start: start2,
                baseline_finish: finish2,
                start_variance_days: 0,
                finish_variance_days: 0,
            },
        );

        let start3 = NaiveDate::from_ymd_opt(2025, 1, 27).unwrap();
        let finish3 = NaiveDate::from_ymd_opt(2025, 1, 29).unwrap();
        tasks.insert(
            "test".to_string(),
            ScheduledTask {
                task_id: "test".to_string(),
                start: start3,
                finish: finish3,
                duration: Duration::days(3),
                assignments: vec![],
                slack: Duration::zero(),
                is_critical: true,
                early_start: start3,
                early_finish: finish3,
                late_start: start3,
                late_finish: finish3,
                forecast_start: start3,
                forecast_finish: finish3,
                remaining_duration: Duration::days(3),
                percent_complete: 0,
                status: TaskStatus::NotStarted,
                cost_range: None,
                has_abstract_assignments: false,
                baseline_start: start3,
                baseline_finish: finish3,
                start_variance_days: 0,
                finish_variance_days: 0,
            },
        );

        let project_end = NaiveDate::from_ymd_opt(2025, 1, 29).unwrap();
        Schedule {
            tasks,
            critical_path: vec![
                "design".to_string(),
                "implement".to_string(),
                "test".to_string(),
            ],
            project_duration: Duration::days(18),
            project_end,
            total_cost: None,
            total_cost_range: None,
            project_progress: 0,
            project_baseline_finish: project_end,
            project_forecast_finish: project_end,
            project_variance_days: 0,
            planned_value: 0,
            earned_value: 0,
            spi: 1.0,
        }
    }

    #[test]
    fn html_gantt_renderer_creation() {
        let renderer = HtmlGanttRenderer::new();
        assert_eq!(renderer.chart_width, 900);
        assert_eq!(renderer.row_height, 32);
        assert!(renderer.interactive);
    }

    #[test]
    fn html_gantt_with_dark_theme() {
        let renderer = HtmlGanttRenderer::new().dark_theme();
        assert_eq!(renderer.theme.background_color, "#1a1a2e");
    }

    #[test]
    fn html_gantt_produces_valid_html() {
        let renderer = HtmlGanttRenderer::new();
        let project = create_test_project();
        let schedule = create_test_schedule();

        let result = renderer.render(&project, &schedule);
        assert!(result.is_ok());

        let html = result.unwrap();
        assert!(html.starts_with("<!DOCTYPE html>"));
        assert!(html.contains("</html>"));
        assert!(html.contains("Test Project"));
        assert!(html.contains("Design Phase"));
    }

    #[test]
    fn html_gantt_includes_svg() {
        let renderer = HtmlGanttRenderer::new();
        let project = create_test_project();
        let schedule = create_test_schedule();

        let html = renderer.render(&project, &schedule).unwrap();
        assert!(html.contains("<svg"));
        assert!(html.contains("</svg>"));
    }

    #[test]
    fn html_gantt_includes_interactivity() {
        let renderer = HtmlGanttRenderer::new();
        let project = create_test_project();
        let schedule = create_test_schedule();

        let html = renderer.render(&project, &schedule).unwrap();
        assert!(html.contains("zoomIn()"));
        assert!(html.contains("tooltip"));
        assert!(html.contains("taskData"));
    }

    #[test]
    fn html_gantt_static_mode() {
        let renderer = HtmlGanttRenderer::new().static_chart();
        let project = create_test_project();
        let schedule = create_test_schedule();

        let html = renderer.render(&project, &schedule).unwrap();
        // Should not have interactive JS
        assert!(!html.contains("taskData"));
    }

    #[test]
    fn html_gantt_empty_schedule_fails() {
        let renderer = HtmlGanttRenderer::new();
        let project = Project::new("Empty");
        let project_end = NaiveDate::from_ymd_opt(2025, 1, 1).unwrap();
        let schedule = Schedule {
            tasks: HashMap::new(),
            critical_path: vec![],
            project_duration: Duration::zero(),
            project_end,
            total_cost: None,
            total_cost_range: None,
            project_progress: 0,
            project_baseline_finish: project_end,
            project_forecast_finish: project_end,
            project_variance_days: 0,
            planned_value: 0,
            earned_value: 0,
            spi: 1.0,
        };

        let result = renderer.render(&project, &schedule);
        assert!(result.is_err());
    }

    #[test]
    fn html_escape_works() {
        assert_eq!(html_escape("<script>"), "&lt;script&gt;");
        assert_eq!(html_escape("a & b"), "a &amp; b");
    }

    #[test]
    fn truncate_works() {
        assert_eq!(truncate("Short", 20), "Short");
        assert_eq!(truncate("This is a very long name", 10), "This is a…");
    }

    #[test]
    fn html_gantt_row_height_option() {
        // Test row_height builder method (lines 123-125)
        let renderer = HtmlGanttRenderer::new().row_height(48);
        assert_eq!(renderer.row_height, 48);
    }

    #[test]
    fn html_gantt_with_ss_dependency() {
        // Test Start-to-Start dependency rendering (lines 623-625)
        let mut project = Project::new("SS Deps");
        project.start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        project
            .tasks
            .push(Task::new("a").name("Task A").effort(Duration::days(5)));
        let mut task_b = Task::new("b").name("Task B").effort(Duration::days(3));
        task_b.depends.push(utf8proj_core::Dependency {
            predecessor: "a".to_string(),
            dep_type: utf8proj_core::DependencyType::StartToStart,
            lag: None,
        });
        project.tasks.push(task_b);

        let start1 = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        let finish1 = NaiveDate::from_ymd_opt(2025, 1, 10).unwrap();
        let finish2 = NaiveDate::from_ymd_opt(2025, 1, 8).unwrap();
        let mut tasks = HashMap::new();
        tasks.insert(
            "a".to_string(),
            ScheduledTask {
                task_id: "a".to_string(),
                start: start1,
                finish: finish1,
                duration: Duration::days(5),
                assignments: vec![],
                slack: Duration::zero(),
                is_critical: true,
                early_start: start1,
                early_finish: finish1,
                late_start: start1,
                late_finish: finish1,
                forecast_start: start1,
                forecast_finish: finish1,
                remaining_duration: Duration::days(5),
                percent_complete: 0,
                status: TaskStatus::NotStarted,
                cost_range: None,
                has_abstract_assignments: false,
                baseline_start: start1,
                baseline_finish: finish1,
                start_variance_days: 0,
                finish_variance_days: 0,
            },
        );
        tasks.insert(
            "b".to_string(),
            ScheduledTask {
                task_id: "b".to_string(),
                start: start1, // SS: starts at same time
                finish: finish2,
                duration: Duration::days(3),
                assignments: vec![],
                slack: Duration::zero(),
                is_critical: false,
                early_start: start1,
                early_finish: finish2,
                late_start: start1,
                late_finish: finish2,
                forecast_start: start1,
                forecast_finish: finish2,
                remaining_duration: Duration::days(3),
                percent_complete: 0,
                status: TaskStatus::NotStarted,
                cost_range: None,
                has_abstract_assignments: false,
                baseline_start: start1,
                baseline_finish: finish2,
                start_variance_days: 0,
                finish_variance_days: 0,
            },
        );

        let schedule = Schedule {
            tasks,
            critical_path: vec!["a".to_string()],
            project_duration: Duration::days(5),
            project_end: finish1,
            total_cost: None,
            total_cost_range: None,
            project_progress: 0,
            project_baseline_finish: finish1,
            project_forecast_finish: finish1,
            project_variance_days: 0,
            planned_value: 0,
            earned_value: 0,
            spi: 1.0,
        };

        let renderer = HtmlGanttRenderer::new();
        let result = renderer.render(&project, &schedule);
        assert!(result.is_ok());
        let html = result.unwrap();
        // Should contain dependency arrow path
        assert!(html.contains("dep-arrow"));
    }

    #[test]
    fn html_gantt_with_ff_dependency() {
        // Test Finish-to-Finish dependency rendering (lines 628-630)
        let mut project = Project::new("FF Deps");
        project.start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        project
            .tasks
            .push(Task::new("a").name("Task A").effort(Duration::days(5)));
        let mut task_b = Task::new("b").name("Task B").effort(Duration::days(3));
        task_b.depends.push(utf8proj_core::Dependency {
            predecessor: "a".to_string(),
            dep_type: utf8proj_core::DependencyType::FinishToFinish,
            lag: None,
        });
        project.tasks.push(task_b);

        let start1 = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        let finish1 = NaiveDate::from_ymd_opt(2025, 1, 10).unwrap();
        let start2 = NaiveDate::from_ymd_opt(2025, 1, 8).unwrap();
        let mut tasks = HashMap::new();
        tasks.insert(
            "a".to_string(),
            ScheduledTask {
                task_id: "a".to_string(),
                start: start1,
                finish: finish1,
                duration: Duration::days(5),
                assignments: vec![],
                slack: Duration::zero(),
                is_critical: true,
                early_start: start1,
                early_finish: finish1,
                late_start: start1,
                late_finish: finish1,
                forecast_start: start1,
                forecast_finish: finish1,
                remaining_duration: Duration::days(5),
                percent_complete: 0,
                status: TaskStatus::NotStarted,
                cost_range: None,
                has_abstract_assignments: false,
                baseline_start: start1,
                baseline_finish: finish1,
                start_variance_days: 0,
                finish_variance_days: 0,
            },
        );
        tasks.insert(
            "b".to_string(),
            ScheduledTask {
                task_id: "b".to_string(),
                start: start2,
                finish: finish1, // FF: finishes at same time
                duration: Duration::days(3),
                assignments: vec![],
                slack: Duration::zero(),
                is_critical: false,
                early_start: start2,
                early_finish: finish1,
                late_start: start2,
                late_finish: finish1,
                forecast_start: start2,
                forecast_finish: finish1,
                remaining_duration: Duration::days(3),
                percent_complete: 0,
                status: TaskStatus::NotStarted,
                cost_range: None,
                has_abstract_assignments: false,
                baseline_start: start2,
                baseline_finish: finish1,
                start_variance_days: 0,
                finish_variance_days: 0,
            },
        );

        let schedule = Schedule {
            tasks,
            critical_path: vec!["a".to_string()],
            project_duration: Duration::days(5),
            project_end: finish1,
            total_cost: None,
            total_cost_range: None,
            project_progress: 0,
            project_baseline_finish: finish1,
            project_forecast_finish: finish1,
            project_variance_days: 0,
            planned_value: 0,
            earned_value: 0,
            spi: 1.0,
        };

        let renderer = HtmlGanttRenderer::new();
        let result = renderer.render(&project, &schedule);
        assert!(result.is_ok());
    }

    #[test]
    fn html_gantt_with_sf_dependency() {
        // Test Start-to-Finish dependency rendering (lines 633-635)
        let mut project = Project::new("SF Deps");
        project.start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        project
            .tasks
            .push(Task::new("a").name("Task A").effort(Duration::days(5)));
        let mut task_b = Task::new("b").name("Task B").effort(Duration::days(3));
        task_b.depends.push(utf8proj_core::Dependency {
            predecessor: "a".to_string(),
            dep_type: utf8proj_core::DependencyType::StartToFinish,
            lag: None,
        });
        project.tasks.push(task_b);

        let start1 = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        let finish1 = NaiveDate::from_ymd_opt(2025, 1, 10).unwrap();
        let start2 = NaiveDate::from_ymd_opt(2025, 1, 3).unwrap();
        let mut tasks = HashMap::new();
        tasks.insert(
            "a".to_string(),
            ScheduledTask {
                task_id: "a".to_string(),
                start: start1,
                finish: finish1,
                duration: Duration::days(5),
                assignments: vec![],
                slack: Duration::zero(),
                is_critical: true,
                early_start: start1,
                early_finish: finish1,
                late_start: start1,
                late_finish: finish1,
                forecast_start: start1,
                forecast_finish: finish1,
                remaining_duration: Duration::days(5),
                percent_complete: 0,
                status: TaskStatus::NotStarted,
                cost_range: None,
                has_abstract_assignments: false,
                baseline_start: start1,
                baseline_finish: finish1,
                start_variance_days: 0,
                finish_variance_days: 0,
            },
        );
        tasks.insert(
            "b".to_string(),
            ScheduledTask {
                task_id: "b".to_string(),
                start: start2,
                finish: start1, // SF: b finishes when a starts
                duration: Duration::days(3),
                assignments: vec![],
                slack: Duration::zero(),
                is_critical: false,
                early_start: start2,
                early_finish: start1,
                late_start: start2,
                late_finish: start1,
                forecast_start: start2,
                forecast_finish: start1,
                remaining_duration: Duration::days(3),
                percent_complete: 0,
                status: TaskStatus::NotStarted,
                cost_range: None,
                has_abstract_assignments: false,
                baseline_start: start2,
                baseline_finish: start1,
                start_variance_days: 0,
                finish_variance_days: 0,
            },
        );

        let schedule = Schedule {
            tasks,
            critical_path: vec!["a".to_string()],
            project_duration: Duration::days(5),
            project_end: finish1,
            total_cost: None,
            total_cost_range: None,
            project_progress: 0,
            project_baseline_finish: finish1,
            project_forecast_finish: finish1,
            project_variance_days: 0,
            planned_value: 0,
            earned_value: 0,
            spi: 1.0,
        };

        let renderer = HtmlGanttRenderer::new();
        let result = renderer.render(&project, &schedule);
        assert!(result.is_ok());
    }

    // =========================================================================
    // Focus View Tests (TDD)
    // =========================================================================

    #[test]
    fn focus_config_empty_patterns_matches_all() {
        let config = FocusConfig::new(vec![], 1);
        assert!(config.matches_focus("any_task", "Any Task Name"));
        assert!(config.matches_focus("6.3.2.1", "[6.3.2.1] GNU Validation"));
    }

    #[test]
    fn focus_config_prefix_matching() {
        let config = FocusConfig::new(vec!["6.3.2".to_string()], 1);

        // Should match: task IDs starting with "6.3.2"
        assert!(config.matches_focus("6.3.2", "Container"));
        assert!(config.matches_focus("6.3.2.1", "Child 1"));
        assert!(config.matches_focus("6.3.2.5.1", "Grandchild"));

        // Should NOT match: different prefixes
        assert!(!config.matches_focus("6.3.1", "Different stream"));
        assert!(!config.matches_focus("7.1", "Different section"));
    }

    #[test]
    fn focus_config_name_matching() {
        let config = FocusConfig::new(vec!["[6.3.2]".to_string()], 1);

        // Should match: task names containing "[6.3.2]"
        assert!(config.matches_focus("task_2259", "[6.3.2] OS Script Migration"));

        // Should NOT match
        assert!(!config.matches_focus("task_2258", "[6.3.1] Something Else"));
    }

    #[test]
    fn focus_config_glob_matching() {
        let config = FocusConfig::new(vec!["*.3.2.*".to_string()], 1);

        // Should match: glob pattern
        assert!(config.matches_focus("6.3.2.1", "Task"));
        assert!(config.matches_focus("7.3.2.5", "Task"));

        // Should NOT match
        assert!(!config.matches_focus("6.4.2.1", "Task"));
    }

    #[test]
    fn focus_config_multiple_patterns() {
        let config = FocusConfig::new(vec!["6.3.2".to_string(), "8.6".to_string()], 1);

        assert!(config.matches_focus("6.3.2.1", "Task"));
        assert!(config.matches_focus("8.6.2", "Task"));
        assert!(!config.matches_focus("7.1", "Task"));
    }

    #[test]
    fn focus_visibility_direct_match() {
        let config = FocusConfig::new(vec!["6.3.2".to_string()], 1);

        let vis = config.get_visibility("6.3.2.1", "Task", 2, false, false);
        assert_eq!(vis, TaskVisibility::Expanded);
    }

    #[test]
    fn focus_visibility_ancestor_of_focused() {
        let config = FocusConfig::new(vec!["6.3.2".to_string()], 1);

        // "6" is ancestor of "6.3.2" - should be expanded to show path
        let vis = config.get_visibility("6", "Section 6", 0, true, false);
        assert_eq!(vis, TaskVisibility::Expanded);
    }

    #[test]
    fn focus_visibility_descendant_of_focused() {
        let config = FocusConfig::new(vec!["6.3".to_string()], 1);

        // "6.3.2.1" is descendant of "6.3" - should be expanded
        let vis = config.get_visibility("6.3.2.1", "Task", 3, false, true);
        assert_eq!(vis, TaskVisibility::Expanded);
    }

    #[test]
    fn focus_visibility_context_depth() {
        let config = FocusConfig::new(vec!["6.3.2".to_string()], 1);

        // Depth 0 (top-level): collapsed (within context_depth)
        let vis = config.get_visibility("7", "Other Section", 0, false, false);
        assert_eq!(vis, TaskVisibility::Collapsed);

        // Depth 1: hidden (exceeds context_depth)
        let vis = config.get_visibility("7.1", "Subsection", 1, false, false);
        assert_eq!(vis, TaskVisibility::Hidden);
    }

    #[test]
    fn focus_visibility_context_depth_zero_hides_all() {
        let config = FocusConfig::new(vec!["6.3.2".to_string()], 0);

        // context_depth=0 means hide all non-focused tasks
        let vis = config.get_visibility("7", "Other Section", 0, false, false);
        assert_eq!(vis, TaskVisibility::Hidden);
    }

    #[test]
    fn focus_visibility_context_depth_two() {
        let config = FocusConfig::new(vec!["6.3.2".to_string()], 2);

        // Depth 0: collapsed
        let vis = config.get_visibility("7", "Other", 0, false, false);
        assert_eq!(vis, TaskVisibility::Collapsed);

        // Depth 1: collapsed (within context_depth=2)
        let vis = config.get_visibility("7.1", "Subsection", 1, false, false);
        assert_eq!(vis, TaskVisibility::Collapsed);

        // Depth 2: hidden
        let vis = config.get_visibility("7.1.1", "Task", 2, false, false);
        assert_eq!(vis, TaskVisibility::Hidden);
    }

    #[test]
    fn focus_config_with_renderer() {
        let mut renderer = HtmlGanttRenderer::new();
        renderer.focus = Some(FocusConfig::new(vec!["6.3.2".to_string()], 1));

        assert!(renderer.focus.is_some());
        assert_eq!(renderer.focus.as_ref().unwrap().focus_patterns.len(), 1);
        assert_eq!(renderer.focus.as_ref().unwrap().context_depth, 1);
    }

    // =========================================================================
    // Now Line Tests (RFC-0017 TDD)
    // =========================================================================

    #[test]
    fn now_line_config_default_disabled() {
        let config = NowLineConfig::default();
        assert!(config.status_date.is_none());
        assert!(!config.show_today);
        assert!(!config.disabled);
    }

    #[test]
    fn now_line_config_with_status_date() {
        let date = NaiveDate::from_ymd_opt(2025, 1, 15).unwrap();
        let config = NowLineConfig::with_status_date(date);
        assert_eq!(config.status_date, Some(date));
        assert!(!config.show_today);
        assert!(!config.disabled);
    }

    #[test]
    fn now_line_config_with_today() {
        let date = NaiveDate::from_ymd_opt(2025, 1, 15).unwrap();
        let config = NowLineConfig::with_status_date(date).with_today();
        assert_eq!(config.status_date, Some(date));
        assert!(config.show_today);
    }

    #[test]
    fn now_line_config_disabled() {
        let config = NowLineConfig::disabled();
        assert!(config.status_date.is_none());
        assert!(config.disabled);
    }

    #[test]
    fn now_line_at_status_date() {
        // Test that now line is rendered at correct position
        let project = create_test_project();
        let schedule = create_test_schedule();

        // Status date is Jan 15, project runs Jan 6-29
        let status_date = NaiveDate::from_ymd_opt(2025, 1, 15).unwrap();
        let renderer =
            HtmlGanttRenderer::new().with_now_line(NowLineConfig::with_status_date(status_date));

        let html = renderer.render(&project, &schedule).unwrap();

        // Should contain the now line SVG element
        assert!(html.contains("now-line"), "Should contain now-line class");
        assert!(
            html.contains("status-date"),
            "Should contain status-date class"
        );
        // Should contain the date label
        assert!(
            html.contains("2025-01-15"),
            "Should contain the status date label"
        );
    }

    #[test]
    fn now_line_outside_range_not_rendered() {
        // Test that now line is NOT rendered when outside chart range
        let project = create_test_project();
        let schedule = create_test_schedule();

        // Status date is Feb 15, but project ends Jan 29
        let status_date = NaiveDate::from_ymd_opt(2025, 2, 15).unwrap();
        let renderer =
            HtmlGanttRenderer::new().with_now_line(NowLineConfig::with_status_date(status_date));

        let html = renderer.render(&project, &schedule).unwrap();

        // Should NOT contain now-line elements since date is outside range
        assert!(
            !html.contains("class=\"now-line"),
            "Should not render now-line outside chart range"
        );
    }

    #[test]
    fn now_line_disabled_not_rendered() {
        // Test that now line is NOT rendered when disabled
        let project = create_test_project();
        let schedule = create_test_schedule();

        let renderer = HtmlGanttRenderer::new().with_now_line(NowLineConfig::disabled());

        let html = renderer.render(&project, &schedule).unwrap();

        // Should NOT contain now-line elements
        assert!(
            !html.contains("class=\"now-line"),
            "Should not render now-line when disabled"
        );
    }

    #[test]
    fn now_line_css_classes_applied() {
        // Test that correct CSS classes are in the stylesheet
        let project = create_test_project();
        let schedule = create_test_schedule();

        let status_date = NaiveDate::from_ymd_opt(2025, 1, 15).unwrap();
        let renderer =
            HtmlGanttRenderer::new().with_now_line(NowLineConfig::with_status_date(status_date));

        let html = renderer.render(&project, &schedule).unwrap();

        // Should contain CSS styling for now-line
        assert!(
            html.contains(".now-line"),
            "Should contain .now-line CSS rule"
        );
    }

    #[test]
    fn now_line_default_not_rendered() {
        // Test that default (no status_date set) doesn't render a line
        let project = create_test_project();
        let schedule = create_test_schedule();

        let renderer = HtmlGanttRenderer::new(); // Default NowLineConfig

        let html = renderer.render(&project, &schedule).unwrap();

        // Default config has no status_date, so no line should be rendered
        assert!(
            !html.contains("class=\"now-line"),
            "Default should not render now-line without status_date"
        );
    }

    #[test]
    fn now_line_builder_method() {
        let date = NaiveDate::from_ymd_opt(2025, 1, 15).unwrap();
        let renderer =
            HtmlGanttRenderer::new().with_now_line(NowLineConfig::with_status_date(date));

        assert_eq!(renderer.now_line.status_date, Some(date));
    }
}
