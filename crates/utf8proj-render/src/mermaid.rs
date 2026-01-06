//! MermaidJS Gantt chart renderer
//!
//! Generates text-based Gantt charts in MermaidJS format, suitable for
//! embedding in Markdown documentation, GitHub, wikis, and other platforms.
//!
//! ## Example Output
//!
//! ```text
//! gantt
//!     title Project Name
//!     dateFormat YYYY-MM-DD
//!
//!     section Phase 1
//!     Design           :crit, t1, 2025-01-06, 5d
//!     Implementation   :t2, after t1, 10d
//!
//!     section Phase 2
//!     Testing          :t3, after t2, 3d
//!     Deployment       :milestone, m1, after t3, 0d
//! ```

use utf8proj_core::{Project, RenderError, Renderer, Schedule, ScheduledTask};

/// MermaidJS Gantt chart renderer
#[derive(Clone, Debug)]
pub struct MermaidRenderer {
    /// Whether to show sections (group by parent task)
    pub show_sections: bool,
    /// Whether to mark critical path tasks
    pub show_critical: bool,
    /// Whether to show task completion status
    pub show_completion: bool,
    /// Date format (MermaidJS format string)
    pub date_format: String,
    /// Whether to use `after` syntax for dependencies
    pub use_dependencies: bool,
    /// Exclude weekends from duration calculation
    pub exclude_weekends: bool,
}

impl Default for MermaidRenderer {
    fn default() -> Self {
        Self {
            show_sections: true,
            show_critical: true,
            show_completion: true,
            date_format: "YYYY-MM-DD".into(),
            use_dependencies: true,
            exclude_weekends: false,
        }
    }
}

impl MermaidRenderer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Disable sections grouping
    pub fn no_sections(mut self) -> Self {
        self.show_sections = false;
        self
    }

    /// Disable critical path highlighting
    pub fn no_critical(mut self) -> Self {
        self.show_critical = false;
        self
    }

    /// Disable completion status
    pub fn no_completion(mut self) -> Self {
        self.show_completion = false;
        self
    }

    /// Use absolute dates instead of `after` dependencies
    pub fn absolute_dates(mut self) -> Self {
        self.use_dependencies = false;
        self
    }

    /// Set custom date format
    pub fn date_format(mut self, format: impl Into<String>) -> Self {
        self.date_format = format.into();
        self
    }

    /// Exclude weekends (use excludes directive)
    pub fn exclude_weekends(mut self) -> Self {
        self.exclude_weekends = true;
        self
    }

    /// Sanitize task name for Mermaid (escape special characters)
    fn sanitize_name(name: &str) -> String {
        // Mermaid is sensitive to colons and special chars in task names
        name.replace(':', "-")
            .replace(';', "-")
            .replace('#', "")
            .replace('\n', " ")
            .replace('\r', "")
    }

    /// Create a valid Mermaid task ID from task_id
    fn make_id(task_id: &str) -> String {
        // Mermaid IDs must be alphanumeric with underscores
        task_id
            .chars()
            .map(|c| if c.is_alphanumeric() || c == '_' { c } else { '_' })
            .collect()
    }

    /// Format duration for Mermaid
    fn format_duration(task: &ScheduledTask) -> String {
        let days = task.duration.as_days().ceil() as i64;
        if days == 0 {
            "0d".into()
        } else {
            format!("{}d", days)
        }
    }

    /// Get task modifiers (crit, done, active, milestone)
    fn get_modifiers(&self, task: &ScheduledTask, complete: Option<f32>) -> Vec<&'static str> {
        let mut mods = Vec::new();

        // Check if milestone
        if task.duration.minutes == 0 {
            mods.push("milestone");
        }

        // Critical path
        if self.show_critical && task.is_critical {
            mods.push("crit");
        }

        // Completion status
        if self.show_completion {
            if let Some(pct) = complete {
                if pct >= 100.0 {
                    mods.push("done");
                } else if pct > 0.0 {
                    mods.push("active");
                }
            }
        }

        mods
    }
}

impl Renderer for MermaidRenderer {
    type Output = String;

    fn render(&self, project: &Project, schedule: &Schedule) -> Result<String, RenderError> {
        if schedule.tasks.is_empty() {
            return Err(RenderError::InvalidData("No tasks to render".into()));
        }

        let mut output = String::new();

        // Header
        output.push_str("gantt\n");
        output.push_str(&format!("    title {}\n", Self::sanitize_name(&project.name)));
        output.push_str(&format!("    dateFormat {}\n", self.date_format));

        // Exclude weekends if enabled
        if self.exclude_weekends {
            output.push_str("    excludes weekends\n");
        }

        output.push('\n');

        // Sort tasks by start date
        let mut tasks: Vec<(&String, &ScheduledTask)> = schedule.tasks.iter().collect();
        tasks.sort_by_key(|(_, t)| t.start);

        // Build dependency map (task_id -> first predecessor)
        let mut first_predecessor: std::collections::HashMap<String, String> =
            std::collections::HashMap::new();
        for task in &project.tasks {
            self.collect_predecessors(task, &mut first_predecessor);
        }

        // Group by section if enabled
        if self.show_sections {
            // Group tasks by their parent (first part of qualified ID)
            let mut sections: std::collections::HashMap<String, Vec<(&String, &ScheduledTask)>> =
                std::collections::HashMap::new();

            for (task_id, scheduled) in &tasks {
                let section = if task_id.contains('.') {
                    task_id.split('.').next().unwrap_or("Tasks").to_string()
                } else {
                    "Tasks".to_string()
                };
                sections
                    .entry(section)
                    .or_default()
                    .push((task_id, scheduled));
            }

            // Sort sections
            let mut section_names: Vec<_> = sections.keys().cloned().collect();
            section_names.sort();

            for section_name in section_names {
                if let Some(section_tasks) = sections.get(&section_name) {
                    // Get section display name from project
                    let display_name = project
                        .get_task(&section_name)
                        .map(|t| t.name.clone())
                        .unwrap_or_else(|| section_name.clone());

                    output.push_str(&format!("    section {}\n", Self::sanitize_name(&display_name)));

                    for (task_id, scheduled) in section_tasks {
                        let line = self.format_task_line(
                            task_id,
                            scheduled,
                            project,
                            &first_predecessor,
                        );
                        output.push_str(&format!("    {}\n", line));
                    }
                    output.push('\n');
                }
            }
        } else {
            // No sections - flat list
            for (task_id, scheduled) in &tasks {
                let line = self.format_task_line(task_id, scheduled, project, &first_predecessor);
                output.push_str(&format!("    {}\n", line));
            }
        }

        Ok(output)
    }
}

impl MermaidRenderer {
    /// Collect first predecessor for each task
    fn collect_predecessors(
        &self,
        task: &utf8proj_core::Task,
        map: &mut std::collections::HashMap<String, String>,
    ) {
        if let Some(first_dep) = task.depends.first() {
            map.insert(task.id.clone(), first_dep.predecessor.clone());
        }
        for child in &task.children {
            self.collect_predecessors(child, map);
        }
    }

    /// Format a single task line
    fn format_task_line(
        &self,
        task_id: &str,
        scheduled: &ScheduledTask,
        project: &Project,
        first_predecessor: &std::collections::HashMap<String, String>,
    ) -> String {
        // Get task info from project
        let task = project.get_task(task_id);
        let name = task
            .map(|t| t.name.clone())
            .unwrap_or_else(|| task_id.to_string());
        let complete = task.and_then(|t| t.complete);

        let sanitized_name = Self::sanitize_name(&name);
        let mermaid_id = Self::make_id(task_id);
        let duration = Self::format_duration(scheduled);
        let modifiers = self.get_modifiers(scheduled, complete);

        // Build the task specification
        let mut parts = Vec::new();

        // Add modifiers (crit, done, active, milestone)
        for m in &modifiers {
            parts.push(m.to_string());
        }

        // Add task ID
        parts.push(mermaid_id.clone());

        // Add start (either "after X" or absolute date)
        if self.use_dependencies {
            if let Some(pred) = first_predecessor.get(task_id) {
                parts.push(format!("after {}", Self::make_id(pred)));
            } else {
                parts.push(scheduled.start.format("%Y-%m-%d").to_string());
            }
        } else {
            parts.push(scheduled.start.format("%Y-%m-%d").to_string());
        }

        // Add duration
        parts.push(duration);

        format!("{} :{}", sanitized_name, parts.join(", "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use std::collections::HashMap;
    use utf8proj_core::{Duration, Schedule, ScheduledTask, Task, TaskStatus};

    fn create_test_project() -> Project {
        let mut project = Project::new("Test Project");
        project.start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        project.tasks.push(Task::new("design").name("Design Phase").effort(Duration::days(5)));
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

        Schedule {
            tasks,
            critical_path: vec![
                "design".to_string(),
                "implement".to_string(),
                "test".to_string(),
            ],
            project_duration: Duration::days(18),
            project_end: NaiveDate::from_ymd_opt(2025, 1, 29).unwrap(),
            total_cost: None,
            total_cost_range: None,
        }
    }

    #[test]
    fn mermaid_renderer_creation() {
        let renderer = MermaidRenderer::new();
        assert!(renderer.show_sections);
        assert!(renderer.show_critical);
        assert_eq!(renderer.date_format, "YYYY-MM-DD");
    }

    #[test]
    fn mermaid_renderer_with_options() {
        let renderer = MermaidRenderer::new()
            .no_sections()
            .no_critical()
            .absolute_dates()
            .exclude_weekends();

        assert!(!renderer.show_sections);
        assert!(!renderer.show_critical);
        assert!(!renderer.use_dependencies);
        assert!(renderer.exclude_weekends);
    }

    #[test]
    fn mermaid_produces_valid_output() {
        let renderer = MermaidRenderer::new();
        let project = create_test_project();
        let schedule = create_test_schedule();

        let result = renderer.render(&project, &schedule);
        assert!(result.is_ok());

        let output = result.unwrap();
        assert!(output.starts_with("gantt\n"));
        assert!(output.contains("title Test Project"));
        assert!(output.contains("dateFormat YYYY-MM-DD"));
    }

    #[test]
    fn mermaid_includes_critical_marker() {
        let renderer = MermaidRenderer::new();
        let project = create_test_project();
        let schedule = create_test_schedule();

        let output = renderer.render(&project, &schedule).unwrap();
        assert!(output.contains("crit"));
    }

    #[test]
    fn mermaid_uses_after_syntax() {
        let renderer = MermaidRenderer::new();
        let project = create_test_project();
        let schedule = create_test_schedule();

        let output = renderer.render(&project, &schedule).unwrap();
        assert!(output.contains("after design"));
        assert!(output.contains("after implement"));
    }

    #[test]
    fn mermaid_absolute_dates_mode() {
        let renderer = MermaidRenderer::new().absolute_dates();
        let project = create_test_project();
        let schedule = create_test_schedule();

        let output = renderer.render(&project, &schedule).unwrap();
        assert!(!output.contains("after "));
        assert!(output.contains("2025-01-06"));
        assert!(output.contains("2025-01-13"));
    }

    #[test]
    fn mermaid_empty_schedule_fails() {
        let renderer = MermaidRenderer::new();
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
    fn mermaid_sanitizes_special_chars() {
        assert_eq!(MermaidRenderer::sanitize_name("Task: Phase 1"), "Task- Phase 1");
        assert_eq!(MermaidRenderer::sanitize_name("Test;Task"), "Test-Task");
        assert_eq!(MermaidRenderer::sanitize_name("Task #1"), "Task 1");
    }

    #[test]
    fn mermaid_makes_valid_ids() {
        assert_eq!(MermaidRenderer::make_id("task1"), "task1");
        assert_eq!(MermaidRenderer::make_id("phase1.design"), "phase1_design");
        assert_eq!(MermaidRenderer::make_id("task-with-dashes"), "task_with_dashes");
    }

    #[test]
    fn mermaid_excludes_weekends() {
        let renderer = MermaidRenderer::new().exclude_weekends();
        let project = create_test_project();
        let schedule = create_test_schedule();

        let output = renderer.render(&project, &schedule).unwrap();
        assert!(output.contains("excludes weekends"));
    }

    #[test]
    fn mermaid_milestone_detection() {
        let mut project = Project::new("Milestone Test");
        project.start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        project.tasks.push(Task::new("done").name("Project Complete").milestone());

        let ms_date = NaiveDate::from_ymd_opt(2025, 1, 10).unwrap();
        let mut tasks = HashMap::new();
        tasks.insert(
            "done".to_string(),
            ScheduledTask {
                task_id: "done".to_string(),
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
            critical_path: vec!["done".to_string()],
            project_duration: Duration::zero(),
            project_end: NaiveDate::from_ymd_opt(2025, 1, 10).unwrap(),
            total_cost: None,
            total_cost_range: None,
        };

        let renderer = MermaidRenderer::new();
        let output = renderer.render(&project, &schedule).unwrap();
        assert!(output.contains("milestone"));
    }

    #[test]
    fn mermaid_no_completion_option() {
        let renderer = MermaidRenderer::new().no_completion();
        assert!(!renderer.show_completion);
    }

    #[test]
    fn mermaid_custom_date_format() {
        let renderer = MermaidRenderer::new().date_format("DD-MM-YYYY");
        assert_eq!(renderer.date_format, "DD-MM-YYYY");

        let project = create_test_project();
        let schedule = create_test_schedule();
        let output = renderer.render(&project, &schedule).unwrap();
        assert!(output.contains("dateFormat DD-MM-YYYY"));
    }

    #[test]
    fn mermaid_no_sections_flat_list() {
        let renderer = MermaidRenderer::new().no_sections();
        let project = create_test_project();
        let schedule = create_test_schedule();

        let output = renderer.render(&project, &schedule).unwrap();
        // Without sections, output should not contain "section" directive
        assert!(!output.contains("section "));
    }

    #[test]
    fn mermaid_done_modifier_for_complete_task() {
        let mut project = Project::new("Progress Test");
        project.start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        project.tasks.push(
            Task::new("complete")
                .name("Completed Task")
                .effort(Duration::days(5))
                .complete(100.0),
        );

        let start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        let finish = NaiveDate::from_ymd_opt(2025, 1, 10).unwrap();
        let mut tasks = HashMap::new();
        tasks.insert(
            "complete".to_string(),
            ScheduledTask {
                task_id: "complete".to_string(),
                start,
                finish,
                duration: Duration::days(5),
                assignments: vec![],
                slack: Duration::zero(),
                is_critical: true,
                early_start: start,
                early_finish: finish,
                late_start: start,
                late_finish: finish,
                forecast_start: start,
                forecast_finish: finish,
                remaining_duration: Duration::zero(),
                percent_complete: 100,
                status: TaskStatus::Complete,
                cost_range: None,
                has_abstract_assignments: false,
                baseline_start: start,
                baseline_finish: finish,
                start_variance_days: 0,
                finish_variance_days: 0,
            },
        );

        let schedule = Schedule {
            tasks,
            critical_path: vec!["complete".to_string()],
            project_duration: Duration::days(5),
            project_end: finish,
            total_cost: None,
            total_cost_range: None,
        };

        let renderer = MermaidRenderer::new();
        let output = renderer.render(&project, &schedule).unwrap();
        assert!(output.contains("done"));
    }

    #[test]
    fn mermaid_active_modifier_for_in_progress_task() {
        let mut project = Project::new("Progress Test");
        project.start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        project.tasks.push(
            Task::new("inprogress")
                .name("In Progress Task")
                .effort(Duration::days(10))
                .complete(50.0),
        );

        let start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        let finish = NaiveDate::from_ymd_opt(2025, 1, 17).unwrap();
        let mut tasks = HashMap::new();
        tasks.insert(
            "inprogress".to_string(),
            ScheduledTask {
                task_id: "inprogress".to_string(),
                start,
                finish,
                duration: Duration::days(10),
                assignments: vec![],
                slack: Duration::zero(),
                is_critical: true,
                early_start: start,
                early_finish: finish,
                late_start: start,
                late_finish: finish,
                forecast_start: start,
                forecast_finish: finish,
                remaining_duration: Duration::days(5),
                percent_complete: 50,
                status: TaskStatus::InProgress,
                cost_range: None,
                has_abstract_assignments: false,
                baseline_start: start,
                baseline_finish: finish,
                start_variance_days: 0,
                finish_variance_days: 0,
            },
        );

        let schedule = Schedule {
            tasks,
            critical_path: vec!["inprogress".to_string()],
            project_duration: Duration::days(10),
            project_end: finish,
            total_cost: None,
            total_cost_range: None,
        };

        let renderer = MermaidRenderer::new();
        let output = renderer.render(&project, &schedule).unwrap();
        assert!(output.contains("active"));
    }

    #[test]
    fn mermaid_no_completion_hides_done_active() {
        let mut project = Project::new("No Completion");
        project.start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        project.tasks.push(
            Task::new("task")
                .name("Task")
                .effort(Duration::days(5))
                .complete(100.0),
        );

        let start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        let finish = NaiveDate::from_ymd_opt(2025, 1, 10).unwrap();
        let mut tasks = HashMap::new();
        tasks.insert(
            "task".to_string(),
            ScheduledTask {
                task_id: "task".to_string(),
                start,
                finish,
                duration: Duration::days(5),
                assignments: vec![],
                slack: Duration::zero(),
                is_critical: false,
                early_start: start,
                early_finish: finish,
                late_start: start,
                late_finish: finish,
                forecast_start: start,
                forecast_finish: finish,
                remaining_duration: Duration::zero(),
                percent_complete: 100,
                status: TaskStatus::Complete,
                cost_range: None,
                has_abstract_assignments: false,
                baseline_start: start,
                baseline_finish: finish,
                start_variance_days: 0,
                finish_variance_days: 0,
            },
        );

        let schedule = Schedule {
            tasks,
            critical_path: vec![],
            project_duration: Duration::days(5),
            project_end: finish,
            total_cost: None,
            total_cost_range: None,
        };

        let renderer = MermaidRenderer::new().no_completion().no_critical();
        let output = renderer.render(&project, &schedule).unwrap();
        // With no_completion, should not have done or active markers
        assert!(!output.contains("done"));
        assert!(!output.contains("active"));
        assert!(!output.contains("crit"));
    }
}
