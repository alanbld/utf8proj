//! # utf8proj-render
//!
//! Rendering backends for utf8proj schedules.
//!
//! This crate provides:
//! - Interactive HTML Gantt chart rendering
//! - SVG Gantt chart rendering
//! - MermaidJS Gantt chart rendering (for Markdown/docs)
//! - PlantUML Gantt chart rendering (for wikis and documentation)
//! - Excel costing reports (for corporate project quoting)
//! - Text-based output
//! - Custom renderer trait
//!
//! ## Example
//!
//! ```rust,ignore
//! use utf8proj_core::{Project, Schedule, Renderer};
//! use utf8proj_render::{HtmlGanttRenderer, SvgRenderer, MermaidRenderer, PlantUmlRenderer};
//!
//! // Interactive HTML Gantt chart
//! let renderer = HtmlGanttRenderer::new();
//! let html = renderer.render(&project, &schedule)?;
//!
//! // Pure SVG output
//! let svg_renderer = SvgRenderer::default();
//! let svg = svg_renderer.render(&project, &schedule)?;
//!
//! // MermaidJS for Markdown/documentation
//! let mermaid_renderer = MermaidRenderer::new();
//! let mermaid = mermaid_renderer.render(&project, &schedule)?;
//!
//! // PlantUML for wikis and documentation
//! let plantuml_renderer = PlantUmlRenderer::new();
//! let plantuml = plantuml_renderer.render(&project, &schedule)?;
//!
//! // Excel costing report
//! let excel_renderer = ExcelRenderer::new().currency("€");
//! let xlsx_bytes = excel_renderer.render(&project, &schedule)?;
//! std::fs::write("project_cost.xlsx", xlsx_bytes)?;
//! ```

pub mod excel;
pub mod gantt;
pub mod mermaid;
pub mod plantuml;

pub use excel::ExcelRenderer;
pub use gantt::{GanttTheme, HtmlGanttRenderer};
pub use mermaid::MermaidRenderer;
pub use plantuml::PlantUmlRenderer;

use chrono::NaiveDate;
use svg::node::element::{Group, Line, Rectangle, Text};
use svg::Document;
use utf8proj_core::{Project, RenderError, Renderer, Schedule, ScheduledTask};

/// SVG Gantt chart renderer configuration
#[derive(Clone, Debug)]
pub struct SvgRenderer {
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
    /// Color for critical path tasks
    pub critical_color: String,
    /// Color for normal tasks
    pub normal_color: String,
    /// Color for milestones
    pub milestone_color: String,
    /// Background color
    pub background_color: String,
    /// Grid line color
    pub grid_color: String,
    /// Text color
    pub text_color: String,
    /// Font family
    pub font_family: String,
    /// Font size in pixels
    pub font_size: u32,
}

impl Default for SvgRenderer {
    fn default() -> Self {
        Self {
            chart_width: 800,
            row_height: 28,
            label_width: 180,
            header_height: 50,
            padding: 20,
            critical_color: "#e74c3c".into(),
            normal_color: "#3498db".into(),
            milestone_color: "#9b59b6".into(),
            background_color: "#ffffff".into(),
            grid_color: "#ecf0f1".into(),
            text_color: "#2c3e50".into(),
            font_family: "system-ui, -apple-system, sans-serif".into(),
            font_size: 12,
        }
    }
}

impl SvgRenderer {
    pub fn new() -> Self {
        Self::default()
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

    /// Calculate the total width of the SVG
    fn total_width(&self) -> u32 {
        self.padding * 2 + self.label_width + self.chart_width
    }

    /// Calculate the total height based on number of tasks
    fn total_height(&self, task_count: usize) -> u32 {
        self.padding * 2 + self.header_height + (task_count as u32 * self.row_height)
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

    /// Create the header with date labels
    fn render_header(
        &self,
        project_start: NaiveDate,
        project_end: NaiveDate,
        px_per_day: f64,
    ) -> Group {
        let mut group = Group::new().set("class", "header");

        // Background for header
        let header_bg = Rectangle::new()
            .set("x", self.padding)
            .set("y", self.padding)
            .set("width", self.label_width + self.chart_width)
            .set("height", self.header_height)
            .set("fill", "#f8f9fa");
        group = group.add(header_bg);

        // Calculate appropriate date interval
        let total_days = (project_end - project_start).num_days();
        let interval_days = if total_days <= 14 {
            1 // Show every day
        } else if total_days <= 60 {
            7 // Show weekly
        } else if total_days <= 180 {
            14 // Show bi-weekly
        } else {
            30 // Show monthly
        };

        // Draw date labels
        let mut current = project_start;
        while current <= project_end {
            let x = self.date_to_x(current, project_start, px_per_day);

            // Vertical grid line
            let line = Line::new()
                .set("x1", x)
                .set("y1", self.padding + self.header_height - 10)
                .set("x2", x)
                .set("y2", self.padding + self.header_height)
                .set("stroke", self.text_color.as_str())
                .set("stroke-width", 1);
            group = group.add(line);

            // Date label
            let label = if interval_days == 1 {
                current.format("%d").to_string()
            } else if interval_days <= 7 {
                current.format("%b %d").to_string()
            } else {
                current.format("%b %d").to_string()
            };

            let text = Text::new(label)
                .set("x", x)
                .set("y", self.padding + self.header_height - 15)
                .set("font-family", self.font_family.as_str())
                .set("font-size", self.font_size - 1)
                .set("fill", self.text_color.as_str())
                .set("text-anchor", "middle");
            group = group.add(text);

            current += chrono::Duration::days(interval_days);
        }

        // Month/Year label at the top
        let month_label = project_start.format("%B %Y").to_string();
        let month_text = Text::new(month_label)
            .set("x", self.padding + self.label_width + self.chart_width / 2)
            .set("y", self.padding + 18)
            .set("font-family", self.font_family.as_str())
            .set("font-size", self.font_size + 2)
            .set("font-weight", "bold")
            .set("fill", self.text_color.as_str())
            .set("text-anchor", "middle");
        group = group.add(month_text);

        group
    }

    /// Render grid lines
    fn render_grid(
        &self,
        task_count: usize,
        project_start: NaiveDate,
        project_end: NaiveDate,
        px_per_day: f64,
    ) -> Group {
        let mut group = Group::new().set("class", "grid");

        let chart_top = self.padding + self.header_height;
        let chart_bottom = chart_top + (task_count as u32 * self.row_height);

        // Horizontal lines for each row
        for i in 0..=task_count {
            let y = chart_top + (i as u32 * self.row_height);
            let line = Line::new()
                .set("x1", self.padding)
                .set("y1", y)
                .set("x2", self.padding + self.label_width + self.chart_width)
                .set("y2", y)
                .set("stroke", self.grid_color.as_str())
                .set("stroke-width", 1);
            group = group.add(line);
        }

        // Vertical lines for days/weeks
        let total_days = (project_end - project_start).num_days();
        let interval = if total_days <= 30 { 1 } else { 7 };

        let mut current = project_start;
        while current <= project_end {
            let x = self.date_to_x(current, project_start, px_per_day);
            let line = Line::new()
                .set("x1", x)
                .set("y1", chart_top)
                .set("x2", x)
                .set("y2", chart_bottom)
                .set("stroke", self.grid_color.as_str())
                .set("stroke-width", 1);
            group = group.add(line);
            current += chrono::Duration::days(interval);
        }

        group
    }

    /// Render a single task bar
    fn render_task(
        &self,
        task: &ScheduledTask,
        task_name: &str,
        row: usize,
        project_start: NaiveDate,
        px_per_day: f64,
    ) -> Group {
        let mut group = Group::new().set("class", "task");

        let y = self.padding + self.header_height + (row as u32 * self.row_height);
        let bar_height = (self.row_height as f64 * 0.6) as u32;
        let bar_y = y + (self.row_height - bar_height) / 2;

        // Task label
        let label = Text::new(truncate(task_name, 22))
            .set("x", self.padding + 8)
            .set("y", y + self.row_height / 2 + 4)
            .set("font-family", self.font_family.as_str())
            .set("font-size", self.font_size)
            .set("fill", self.text_color.as_str());
        group = group.add(label);

        // Calculate bar position and width
        let x_start = self.date_to_x(task.start, project_start, px_per_day);
        let x_end = self.date_to_x(task.finish, project_start, px_per_day);
        let bar_width = (x_end - x_start).max(4.0); // Minimum width for visibility

        // Determine if this is a milestone (zero duration)
        let is_milestone = task.duration.minutes == 0;

        if is_milestone {
            // Draw diamond for milestone
            let cx = x_start;
            let cy = (bar_y + bar_height / 2) as f64;
            let size = (bar_height as f64) / 2.0;

            let diamond = svg::node::element::Polygon::new()
                .set(
                    "points",
                    format!(
                        "{},{} {},{} {},{} {},{}",
                        cx,
                        cy - size,
                        cx + size,
                        cy,
                        cx,
                        cy + size,
                        cx - size,
                        cy
                    ),
                )
                .set("fill", self.milestone_color.as_str());
            group = group.add(diamond);
        } else {
            // Draw bar for regular task
            let color = if task.is_critical {
                self.critical_color.as_str()
            } else {
                self.normal_color.as_str()
            };

            let bar = Rectangle::new()
                .set("x", x_start)
                .set("y", bar_y)
                .set("width", bar_width)
                .set("height", bar_height)
                .set("rx", 3)
                .set("ry", 3)
                .set("fill", color);
            group = group.add(bar);

            // Add subtle gradient effect
            let highlight = Rectangle::new()
                .set("x", x_start)
                .set("y", bar_y)
                .set("width", bar_width)
                .set("height", bar_height / 3)
                .set("rx", 3)
                .set("ry", 3)
                .set("fill", "rgba(255,255,255,0.2)");
            group = group.add(highlight);
        }

        group
    }

    /// Render the legend
    fn render_legend(&self, y_offset: u32) -> Group {
        let mut group = Group::new().set("class", "legend");
        let x_start = self.padding as f64;
        let y = y_offset as f64 + 15.0;
        let box_size = 12.0;
        let spacing = 120.0;

        // Critical path
        let critical_box = Rectangle::new()
            .set("x", x_start)
            .set("y", y - box_size + 2.0)
            .set("width", box_size)
            .set("height", box_size)
            .set("rx", 2)
            .set("fill", self.critical_color.as_str());
        group = group.add(critical_box);

        let critical_label = Text::new("Critical Path")
            .set("x", x_start + box_size + 5.0)
            .set("y", y)
            .set("font-family", self.font_family.as_str())
            .set("font-size", self.font_size - 1)
            .set("fill", self.text_color.as_str());
        group = group.add(critical_label);

        // Normal task
        let normal_box = Rectangle::new()
            .set("x", x_start + spacing)
            .set("y", y - box_size + 2.0)
            .set("width", box_size)
            .set("height", box_size)
            .set("rx", 2)
            .set("fill", self.normal_color.as_str());
        group = group.add(normal_box);

        let normal_label = Text::new("Normal Task")
            .set("x", x_start + spacing + box_size + 5.0)
            .set("y", y)
            .set("font-family", self.font_family.as_str())
            .set("font-size", self.font_size - 1)
            .set("fill", self.text_color.as_str());
        group = group.add(normal_label);

        // Milestone
        let mx = x_start + spacing * 2.0 + box_size / 2.0;
        let my = y - box_size / 2.0 + 2.0;
        let msize = box_size / 2.0;

        let milestone = svg::node::element::Polygon::new()
            .set(
                "points",
                format!(
                    "{},{} {},{} {},{} {},{}",
                    mx,
                    my - msize,
                    mx + msize,
                    my,
                    mx,
                    my + msize,
                    mx - msize,
                    my
                ),
            )
            .set("fill", self.milestone_color.as_str());
        group = group.add(milestone);

        let milestone_label = Text::new("Milestone")
            .set("x", x_start + spacing * 2.0 + box_size + 5.0)
            .set("y", y)
            .set("font-family", self.font_family.as_str())
            .set("font-size", self.font_size - 1)
            .set("fill", self.text_color.as_str());
        group = group.add(milestone_label);

        group
    }
}

impl Renderer for SvgRenderer {
    type Output = String;

    fn render(&self, project: &Project, schedule: &Schedule) -> Result<String, RenderError> {
        // Sort tasks by start date
        let mut tasks: Vec<&ScheduledTask> = schedule.tasks.values().collect();
        tasks.sort_by_key(|t| t.start);

        if tasks.is_empty() {
            return Err(RenderError::InvalidData("No tasks to render".into()));
        }

        let task_count = tasks.len();
        let project_start = project.start;
        let project_end = schedule.project_end;
        let px_per_day = self.pixels_per_day(project_start, project_end);

        // Calculate dimensions
        let width = self.total_width();
        let height = self.total_height(task_count) + 30; // Extra space for legend

        // Create document
        let mut document = Document::new()
            .set("width", width)
            .set("height", height)
            .set("viewBox", (0, 0, width, height))
            .set("xmlns", "http://www.w3.org/2000/svg");

        // Background
        let background = Rectangle::new()
            .set("width", "100%")
            .set("height", "100%")
            .set("fill", self.background_color.as_str());
        document = document.add(background);

        // Title
        let title = Text::new(project.name.as_str())
            .set("x", self.padding)
            .set("y", self.padding + 15)
            .set("font-family", self.font_family.as_str())
            .set("font-size", self.font_size + 4)
            .set("font-weight", "bold")
            .set("fill", self.text_color.as_str());
        document = document.add(title);

        // Grid
        document = document.add(self.render_grid(task_count, project_start, project_end, px_per_day));

        // Header
        document = document.add(self.render_header(project_start, project_end, px_per_day));

        // Task bars
        for (row, scheduled_task) in tasks.iter().enumerate() {
            // Get the task name from the project
            let task_name = project
                .get_task(&scheduled_task.task_id)
                .map(|t| t.name.as_str())
                .unwrap_or(&scheduled_task.task_id);

            document = document.add(self.render_task(
                scheduled_task,
                task_name,
                row,
                project_start,
                px_per_day,
            ));
        }

        // Legend
        let legend_y = self.padding + self.header_height + (task_count as u32 * self.row_height) + 10;
        document = document.add(self.render_legend(legend_y));

        // Convert to string
        let mut output = Vec::new();
        svg::write(&mut output, &document)
            .map_err(|e| RenderError::Format(format!("Failed to write SVG: {}", e)))?;

        String::from_utf8(output).map_err(|e| RenderError::Format(format!("Invalid UTF-8: {}", e)))
    }
}

/// Truncate a string to a maximum length with ellipsis
fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}...", &s[..max.saturating_sub(3)])
    }
}

/// Plain text renderer for console output
#[derive(Default)]
pub struct TextRenderer;

impl Renderer for TextRenderer {
    type Output = String;

    fn render(&self, project: &Project, _schedule: &Schedule) -> Result<String, RenderError> {
        Ok(format!("Project: {}\n", project.name))
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
        project.start = NaiveDate::from_ymd_opt(2025, 1, 1).unwrap();
        project.tasks.push(
            utf8proj_core::Task::new("task1")
                .name("Design Phase")
                .duration(Duration::days(3)),
        );
        project.tasks.push(
            utf8proj_core::Task::new("task2")
                .name("Implementation")
                .duration(Duration::days(5))
                .depends_on("task1"),
        );
        project
    }

    fn create_test_schedule() -> Schedule {
        let mut tasks = HashMap::new();

        let start1 = NaiveDate::from_ymd_opt(2025, 1, 1).unwrap();
        let finish1 = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        tasks.insert(
            "task1".to_string(),
            ScheduledTask {
                task_id: "task1".to_string(),
                start: start1,
                finish: finish1,
                duration: Duration::days(3),
                assignments: vec![],
                slack: Duration::zero(),
                is_critical: true,
                early_start: start1,
                early_finish: finish1,
                late_start: start1,
                late_finish: finish1,
                forecast_start: start1,
                forecast_finish: finish1,
                remaining_duration: Duration::days(3),
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

        let start2 = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        let finish2 = NaiveDate::from_ymd_opt(2025, 1, 13).unwrap();
        tasks.insert(
            "task2".to_string(),
            ScheduledTask {
                task_id: "task2".to_string(),
                start: start2,
                finish: finish2,
                duration: Duration::days(5),
                assignments: vec![],
                slack: Duration::zero(),
                is_critical: true,
                early_start: start2,
                early_finish: finish2,
                late_start: start2,
                late_finish: finish2,
                forecast_start: start2,
                forecast_finish: finish2,
                remaining_duration: Duration::days(5),
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

        Schedule {
            tasks,
            critical_path: vec!["task1".to_string(), "task2".to_string()],
            project_duration: Duration::days(8),
            project_end: NaiveDate::from_ymd_opt(2025, 1, 13).unwrap(),
            total_cost: None,
            total_cost_range: None,
        }
    }

    #[test]
    fn svg_renderer_creation() {
        let renderer = SvgRenderer::new();
        assert_eq!(renderer.chart_width, 800);
        assert_eq!(renderer.row_height, 28);
    }

    #[test]
    fn svg_renderer_with_config() {
        let renderer = SvgRenderer::new().chart_width(1000).row_height(40);
        assert_eq!(renderer.chart_width, 1000);
        assert_eq!(renderer.row_height, 40);
    }

    #[test]
    fn svg_render_produces_valid_svg() {
        let renderer = SvgRenderer::new();
        let project = create_test_project();
        let schedule = create_test_schedule();

        let result = renderer.render(&project, &schedule);
        assert!(result.is_ok());

        let svg = result.unwrap();
        assert!(svg.starts_with("<svg"));
        assert!(svg.contains("</svg>"));
        assert!(svg.contains("Test Project"));
        assert!(svg.contains("Design Phase"));
    }

    #[test]
    fn svg_render_includes_critical_path_styling() {
        let renderer = SvgRenderer::new();
        let project = create_test_project();
        let schedule = create_test_schedule();

        let svg = renderer.render(&project, &schedule).unwrap();
        assert!(svg.contains(&renderer.critical_color));
    }

    #[test]
    fn svg_render_empty_schedule_fails() {
        let renderer = SvgRenderer::new();
        let project = Project::new("Empty");
        let schedule = Schedule {
            tasks: HashMap::new(),
            critical_path: vec![],
            project_duration: Duration::zero(),
            project_end: NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
            total_cost: None,
            total_cost_range: None,
        };

        let result = renderer.render(&project, &schedule);
        assert!(result.is_err());
    }

    #[test]
    fn truncate_long_string() {
        assert_eq!(truncate("Short", 20), "Short");
        assert_eq!(truncate("This is a very long task name", 15), "This is a ve...");
    }

    #[test]
    fn text_renderer_basic() {
        let renderer = TextRenderer;
        let project = create_test_project();
        let schedule = create_test_schedule();

        let result = renderer.render(&project, &schedule);
        assert!(result.is_ok());
        let text = result.unwrap();
        assert!(text.contains("Test Project"));
    }

    #[test]
    fn svg_render_with_milestone() {
        let mut project = Project::new("Milestone Test");
        project.start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        project.tasks.push(
            utf8proj_core::Task::new("dev")
                .name("Development")
                .duration(Duration::days(5)),
        );
        project.tasks.push(
            utf8proj_core::Task::new("release")
                .name("Release")
                .milestone()
                .depends_on("dev"),
        );

        let mut tasks = HashMap::new();
        let start1 = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        let finish1 = NaiveDate::from_ymd_opt(2025, 1, 10).unwrap();
        tasks.insert(
            "dev".to_string(),
            ScheduledTask {
                task_id: "dev".to_string(),
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
        let ms_date = NaiveDate::from_ymd_opt(2025, 1, 13).unwrap();
        tasks.insert(
            "release".to_string(),
            ScheduledTask {
                task_id: "release".to_string(),
                start: ms_date,
                finish: ms_date,
                duration: Duration::zero(),
                assignments: vec![],
                slack: Duration::zero(),
                is_critical: true,
                early_start: ms_date,
                early_finish: ms_date,
                late_start: ms_date,
                late_finish: ms_date,
                forecast_start: ms_date,
                forecast_finish: ms_date,
                remaining_duration: Duration::zero(),
                percent_complete: 0,
                status: TaskStatus::NotStarted,
                cost_range: None,
                has_abstract_assignments: false,
                baseline_start: ms_date,
                baseline_finish: ms_date,
                start_variance_days: 0,
                finish_variance_days: 0,
            },
        );

        let schedule = Schedule {
            tasks,
            critical_path: vec!["dev".to_string(), "release".to_string()],
            project_duration: Duration::days(5),
            project_end: ms_date,
            total_cost: None,
            total_cost_range: None,
        };

        let renderer = SvgRenderer::new();
        let svg = renderer.render(&project, &schedule).unwrap();

        // Milestone should render as polygon (diamond)
        assert!(svg.contains("polygon"));
        assert!(svg.contains("Release"));
    }

    #[test]
    fn svg_render_non_critical_tasks() {
        let mut project = Project::new("Non-Critical");
        project.start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        project.tasks.push(
            utf8proj_core::Task::new("task1")
                .name("Task 1")
                .duration(Duration::days(5)),
        );

        let mut tasks = HashMap::new();
        let start1 = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        let finish1 = NaiveDate::from_ymd_opt(2025, 1, 10).unwrap();
        tasks.insert(
            "task1".to_string(),
            ScheduledTask {
                task_id: "task1".to_string(),
                start: start1,
                finish: finish1,
                duration: Duration::days(5),
                assignments: vec![],
                slack: Duration::days(5), // Has slack, so not critical
                is_critical: false,
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

        let schedule = Schedule {
            tasks,
            critical_path: vec![],
            project_duration: Duration::days(5),
            project_end: finish1,
            total_cost: None,
            total_cost_range: None,
        };

        let renderer = SvgRenderer::new();
        let svg = renderer.render(&project, &schedule).unwrap();

        // Non-critical tasks should use normal color
        assert!(svg.contains(&renderer.normal_color));
    }

    #[test]
    fn svg_render_long_project() {
        // Test project > 60 days to trigger bi-weekly interval
        let mut project = Project::new("Long Project");
        project.start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        project.tasks.push(
            utf8proj_core::Task::new("phase1")
                .name("Phase 1")
                .duration(Duration::days(50)),
        );
        project.tasks.push(
            utf8proj_core::Task::new("phase2")
                .name("Phase 2")
                .duration(Duration::days(50))
                .depends_on("phase1"),
        );

        let mut tasks = HashMap::new();
        let start1 = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        let finish1 = NaiveDate::from_ymd_opt(2025, 3, 14).unwrap(); // ~50 working days
        tasks.insert(
            "phase1".to_string(),
            ScheduledTask {
                task_id: "phase1".to_string(),
                start: start1,
                finish: finish1,
                duration: Duration::days(50),
                assignments: vec![],
                slack: Duration::zero(),
                is_critical: true,
                early_start: start1,
                early_finish: finish1,
                late_start: start1,
                late_finish: finish1,
                forecast_start: start1,
                forecast_finish: finish1,
                remaining_duration: Duration::days(50),
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
        let start2 = NaiveDate::from_ymd_opt(2025, 3, 17).unwrap();
        let finish2 = NaiveDate::from_ymd_opt(2025, 5, 23).unwrap();
        tasks.insert(
            "phase2".to_string(),
            ScheduledTask {
                task_id: "phase2".to_string(),
                start: start2,
                finish: finish2,
                duration: Duration::days(50),
                assignments: vec![],
                slack: Duration::zero(),
                is_critical: true,
                early_start: start2,
                early_finish: finish2,
                late_start: start2,
                late_finish: finish2,
                forecast_start: start2,
                forecast_finish: finish2,
                remaining_duration: Duration::days(50),
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

        let schedule = Schedule {
            tasks,
            critical_path: vec!["phase1".to_string(), "phase2".to_string()],
            project_duration: Duration::days(100),
            project_end: finish2,
            total_cost: None,
            total_cost_range: None,
        };

        let renderer = SvgRenderer::new();
        let svg = renderer.render(&project, &schedule).unwrap();

        assert!(svg.contains("Long Project"));
        assert!(svg.contains("Phase 1"));
    }

    #[test]
    fn svg_render_very_long_project() {
        // Test project > 180 days to trigger monthly interval
        let mut project = Project::new("Very Long Project");
        project.start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        project.tasks.push(
            utf8proj_core::Task::new("year")
                .name("Year Long Task")
                .duration(Duration::days(200)),
        );

        let mut tasks = HashMap::new();
        let start1 = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        let finish1 = NaiveDate::from_ymd_opt(2025, 10, 31).unwrap();
        tasks.insert(
            "year".to_string(),
            ScheduledTask {
                task_id: "year".to_string(),
                start: start1,
                finish: finish1,
                duration: Duration::days(200),
                assignments: vec![],
                slack: Duration::zero(),
                is_critical: true,
                early_start: start1,
                early_finish: finish1,
                late_start: start1,
                late_finish: finish1,
                forecast_start: start1,
                forecast_finish: finish1,
                remaining_duration: Duration::days(200),
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

        let schedule = Schedule {
            tasks,
            critical_path: vec!["year".to_string()],
            project_duration: Duration::days(200),
            project_end: finish1,
            total_cost: None,
            total_cost_range: None,
        };

        let renderer = SvgRenderer::new();
        let svg = renderer.render(&project, &schedule).unwrap();

        assert!(svg.contains("Very Long Project"));
    }
}
