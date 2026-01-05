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
            label_width: 220,
            header_height: 60,
            padding: 20,
            theme: GanttTheme::default(),
            show_dependencies: true,
            interactive: true,
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

    /// Build flat list of tasks with hierarchy info
    fn flatten_tasks_for_display<'a>(
        &self,
        project: &'a Project,
        schedule: &'a Schedule,
    ) -> Vec<TaskDisplay<'a>> {
        let mut result = Vec::new();
        self.collect_tasks(&project.tasks, schedule, "", 0, &mut result);
        result
    }

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
        let project_start = project.start;
        let project_end = schedule.project_end;
        let px_per_day = self.pixels_per_day(project_start, project_end);

        let total_width = self.padding * 2 + self.label_width + self.chart_width;
        let total_height = self.padding * 2 + self.header_height + (tasks.len() as u32 * self.row_height) + 50;

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
            svg.push_str(&self.render_dependencies(project, schedule, tasks, project_start, px_per_day));
        }

        svg
    }

    /// Render the timeline header
    fn render_header(&self, project_start: NaiveDate, project_end: NaiveDate, px_per_day: f64) -> String {
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
    fn render_grid(&self, task_count: usize, project_start: NaiveDate, project_end: NaiveDate, px_per_day: f64) -> String {
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
    fn render_task_row(&self, task_display: &TaskDisplay, row: usize, project_start: NaiveDate, px_per_day: f64) -> String {
        let mut svg = String::new();

        let y = self.padding + self.header_height + (row as u32 * self.row_height);
        let bar_height = (self.row_height as f64 * 0.6) as u32;
        let bar_y = y + (self.row_height - bar_height) / 2;

        // Indent for hierarchy
        let indent = task_display.depth as u32 * 16;
        let label_x = self.padding + 8 + indent;

        // Container expand/collapse icon
        if task_display.is_container {
            let icon_x = label_x - 12;
            let icon_y = y + self.row_height / 2;
            svg.push_str(&format!(
                r#"                <text x="{x}" y="{y}" font-size="10" fill="{color}" class="collapse-icon" data-task="{id}" style="cursor:pointer">▼</text>"#,
                x = icon_x,
                y = icon_y + 4,
                color = self.theme.text_color,
                id = task_display.qualified_id
            ));
            svg.push('\n');
        }

        // Task label
        let label = truncate(&task_display.task.name, 20 - task_display.depth * 2);
        svg.push_str(&format!(
            r#"                <text x="{x}" y="{y}" font-size="12" fill="{color}">{label}</text>"#,
            x = label_x,
            y = y + self.row_height / 2 + 4,
            color = self.theme.text_color,
            label = html_escape(&label)
        ));
        svg.push('\n');

        // Task bar (if scheduled)
        if let Some(scheduled) = task_display.scheduled {
            let x_start = self.date_to_x(scheduled.start, project_start, px_per_day);
            let x_end = self.date_to_x(scheduled.finish, project_start, px_per_day);
            let bar_width = (x_end - x_start).max(4.0);

            let is_milestone = scheduled.duration.minutes == 0;

            if is_milestone {
                // Diamond for milestone
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
            } else if task_display.is_container {
                // Container bar (bracket style)
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
                let color = if scheduled.is_critical {
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
                    let pred_id = self.resolve_dependency(&dep.predecessor, &task_display.qualified_id, &task_positions);

                    if let Some((from_row, from_scheduled)) = pred_id.and_then(|id| task_positions.get(id.as_str())) {
                        let to_row = task_positions.get(task_display.qualified_id.as_str()).map(|(r, _)| *r).unwrap_or(0);

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
    fn resolve_dependency(&self, dep_path: &str, from_id: &str, positions: &HashMap<&str, (usize, &ScheduledTask)>) -> Option<String> {
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

        let from_y = self.padding as f64 + self.header_height as f64 + (from_row as f64 * self.row_height as f64) + bar_y_offset + bar_height / 2.0;
        let to_y = self.padding as f64 + self.header_height as f64 + (to_row as f64 * self.row_height as f64) + bar_y_offset + bar_height / 2.0;

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
                from_x, from_y,
                mid_x, from_y,
                mid_x, to_y,
                to_x, to_y
            )
        } else {
            // More complex path for distant rows
            let offset = 15.0;
            format!(
                "M{},{} L{},{} L{},{} L{},{}",
                from_x, from_y,
                from_x + offset, from_y,
                from_x + offset, to_y,
                to_x, to_y
            )
        };

        format!(
            r#"                    <path d="{path}" fill="none" stroke="{color}" stroke-width="1.5" marker-end="url(#arrowhead)" class="dep-arrow"/>
"#,
            path = path,
            color = self.theme.arrow_color
        )
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
        }}"#,
            critical = self.theme.critical_color,
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
        format!("{}…", s.chars().take(max.saturating_sub(1)).collect::<String>())
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
            },
        );

        Schedule {
            tasks,
            critical_path: vec!["design".to_string(), "implement".to_string(), "test".to_string()],
            project_duration: Duration::days(18),
            project_end: NaiveDate::from_ymd_opt(2025, 1, 29).unwrap(),
            total_cost: None,
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
        let schedule = Schedule {
            tasks: HashMap::new(),
            critical_path: vec![],
            project_duration: Duration::zero(),
            project_end: NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
            total_cost: None,
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
        project.tasks.push(Task::new("a").name("Task A").effort(Duration::days(5)));
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
            },
        );

        let schedule = Schedule {
            tasks,
            critical_path: vec!["a".to_string()],
            project_duration: Duration::days(5),
            project_end: finish1,
            total_cost: None,
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
        project.tasks.push(Task::new("a").name("Task A").effort(Duration::days(5)));
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
            },
        );

        let schedule = Schedule {
            tasks,
            critical_path: vec!["a".to_string()],
            project_duration: Duration::days(5),
            project_end: finish1,
            total_cost: None,
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
        project.tasks.push(Task::new("a").name("Task A").effort(Duration::days(5)));
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
            },
        );

        let schedule = Schedule {
            tasks,
            critical_path: vec!["a".to_string()],
            project_duration: Duration::days(5),
            project_end: finish1,
            total_cost: None,
        };

        let renderer = HtmlGanttRenderer::new();
        let result = renderer.render(&project, &schedule);
        assert!(result.is_ok());
    }
}
