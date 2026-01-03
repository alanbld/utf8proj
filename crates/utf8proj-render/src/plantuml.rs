//! PlantUML Gantt chart renderer
//!
//! Generates text-based Gantt charts in PlantUML format, suitable for
//! documentation, wikis, and integration with PlantUML rendering tools.
//!
//! ## Example Output
//!
//! ```text
//! @startgantt
//! Project starts 2025-01-06
//! saturday are closed
//! sunday are closed
//!
//! [Design Phase] as [design] starts 2025-01-06 and lasts 5 days
//! [Design Phase] is colored in OrangeRed
//! [Implementation] as [impl] starts at [design]'s end and lasts 10 days
//! [Implementation] is colored in OrangeRed
//! [Testing] as [test] starts at [impl]'s end and lasts 3 days
//! [Deployment] happens at [test]'s end
//! @endgantt
//! ```

use std::collections::HashMap;
use utf8proj_core::{Project, RenderError, Renderer, Schedule, ScheduledTask};

/// PlantUML Gantt chart renderer
#[derive(Clone, Debug)]
pub struct PlantUmlRenderer {
    /// Whether to mark critical path tasks with color
    pub show_critical: bool,
    /// Color for critical path tasks
    pub critical_color: String,
    /// Color for normal tasks
    pub normal_color: String,
    /// Whether to show task completion status
    pub show_completion: bool,
    /// Whether to use `starts at [X]'s end` syntax for dependencies
    pub use_dependencies: bool,
    /// Whether to close weekends
    pub close_weekends: bool,
    /// Whether to show task aliases (shorter IDs)
    pub show_aliases: bool,
    /// Custom scale (day, week, month)
    pub scale: Option<String>,
    /// Whether to use today marker
    pub show_today: bool,
}

impl Default for PlantUmlRenderer {
    fn default() -> Self {
        Self {
            show_critical: true,
            critical_color: "OrangeRed".into(),
            normal_color: "SteelBlue".into(),
            show_completion: true,
            use_dependencies: true,
            close_weekends: true,
            show_aliases: true,
            scale: None,
            show_today: false,
        }
    }
}

impl PlantUmlRenderer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Disable critical path coloring
    pub fn no_critical(mut self) -> Self {
        self.show_critical = false;
        self
    }

    /// Set critical path color
    pub fn critical_color(mut self, color: impl Into<String>) -> Self {
        self.critical_color = color.into();
        self
    }

    /// Set normal task color
    pub fn normal_color(mut self, color: impl Into<String>) -> Self {
        self.normal_color = color.into();
        self
    }

    /// Disable completion status
    pub fn no_completion(mut self) -> Self {
        self.show_completion = false;
        self
    }

    /// Use absolute dates instead of dependency syntax
    pub fn absolute_dates(mut self) -> Self {
        self.use_dependencies = false;
        self
    }

    /// Don't close weekends
    pub fn include_weekends(mut self) -> Self {
        self.close_weekends = false;
        self
    }

    /// Disable task aliases
    pub fn no_aliases(mut self) -> Self {
        self.show_aliases = false;
        self
    }

    /// Set zoom scale (day, week, month)
    pub fn scale(mut self, scale: impl Into<String>) -> Self {
        self.scale = Some(scale.into());
        self
    }

    /// Show today marker
    pub fn show_today(mut self) -> Self {
        self.show_today = true;
        self
    }

    /// Sanitize task name for PlantUML (escape special characters)
    fn sanitize_name(name: &str) -> String {
        // PlantUML uses square brackets for task names
        name.replace('[', "(")
            .replace(']', ")")
            .replace('\n', " ")
            .replace('\r', "")
    }

    /// Create a valid PlantUML alias from task_id
    fn make_alias(task_id: &str) -> String {
        task_id
            .chars()
            .map(|c| if c.is_alphanumeric() || c == '_' { c } else { '_' })
            .collect()
    }

    /// Format duration for PlantUML
    fn format_duration(task: &ScheduledTask) -> String {
        let days = task.duration.as_days().ceil() as i64;
        if days == 0 {
            "0 days".into()
        } else if days == 1 {
            "1 day".into()
        } else {
            format!("{} days", days)
        }
    }
}

impl Renderer for PlantUmlRenderer {
    type Output = String;

    fn render(&self, project: &Project, schedule: &Schedule) -> Result<String, RenderError> {
        if schedule.tasks.is_empty() {
            return Err(RenderError::InvalidData("No tasks to render".into()));
        }

        let mut output = String::new();

        // Header
        output.push_str("@startgantt\n");

        // Project start
        output.push_str(&format!("Project starts {}\n", project.start.format("%Y-%m-%d")));

        // Scale if specified
        if let Some(ref scale) = self.scale {
            output.push_str(&format!("printscale {}\n", scale));
        }

        // Close weekends
        if self.close_weekends {
            output.push_str("saturday are closed\n");
            output.push_str("sunday are closed\n");
        }

        // Today marker
        if self.show_today {
            output.push_str("today is colored in LightBlue\n");
        }

        output.push('\n');

        // Sort tasks by start date
        let mut tasks: Vec<(&String, &ScheduledTask)> = schedule.tasks.iter().collect();
        tasks.sort_by_key(|(_, t)| t.start);

        // Build dependency map (task_id -> first predecessor)
        let mut first_predecessor: HashMap<String, String> = HashMap::new();
        for task in &project.tasks {
            self.collect_predecessors(task, &mut first_predecessor);
        }

        // Track which tasks have been rendered (for dependency references)
        let mut rendered_tasks: std::collections::HashSet<String> = std::collections::HashSet::new();

        // Render each task
        for (task_id, scheduled) in &tasks {
            let task = project.get_task(task_id);
            let name = task
                .map(|t| t.name.clone())
                .unwrap_or_else(|| (*task_id).clone());
            let complete = task.and_then(|t| t.complete);

            let sanitized_name = Self::sanitize_name(&name);
            let alias = Self::make_alias(task_id);
            let is_milestone = scheduled.duration.minutes == 0;

            if is_milestone {
                // Milestone syntax
                if self.use_dependencies {
                    if let Some(pred) = first_predecessor.get(*task_id) {
                        let pred_alias = Self::make_alias(pred);
                        if rendered_tasks.contains(pred) {
                            output.push_str(&format!(
                                "[{}] happens at [{}]'s end\n",
                                sanitized_name, pred_alias
                            ));
                        } else {
                            // Predecessor not rendered yet, use absolute date
                            output.push_str(&format!(
                                "[{}] happens {}\n",
                                sanitized_name,
                                scheduled.start.format("%Y-%m-%d")
                            ));
                        }
                    } else {
                        output.push_str(&format!(
                            "[{}] happens {}\n",
                            sanitized_name,
                            scheduled.start.format("%Y-%m-%d")
                        ));
                    }
                } else {
                    output.push_str(&format!(
                        "[{}] happens {}\n",
                        sanitized_name,
                        scheduled.start.format("%Y-%m-%d")
                    ));
                }
            } else {
                // Regular task
                let duration = Self::format_duration(scheduled);

                // Task definition with optional alias
                let task_def = if self.show_aliases {
                    format!("[{}] as [{}]", sanitized_name, alias)
                } else {
                    format!("[{}]", sanitized_name)
                };

                // Start specification
                if self.use_dependencies {
                    if let Some(pred) = first_predecessor.get(*task_id) {
                        let pred_alias = Self::make_alias(pred);
                        if rendered_tasks.contains(pred) {
                            output.push_str(&format!(
                                "{} starts at [{}]'s end and lasts {}\n",
                                task_def, pred_alias, duration
                            ));
                        } else {
                            // Predecessor not rendered yet, use absolute date
                            output.push_str(&format!(
                                "{} starts {} and lasts {}\n",
                                task_def,
                                scheduled.start.format("%Y-%m-%d"),
                                duration
                            ));
                        }
                    } else {
                        output.push_str(&format!(
                            "{} starts {} and lasts {}\n",
                            task_def,
                            scheduled.start.format("%Y-%m-%d"),
                            duration
                        ));
                    }
                } else {
                    output.push_str(&format!(
                        "{} starts {} and lasts {}\n",
                        task_def,
                        scheduled.start.format("%Y-%m-%d"),
                        duration
                    ));
                }
            }

            // Color for critical path
            if self.show_critical && scheduled.is_critical && !is_milestone {
                let ref_name = if self.show_aliases {
                    alias.clone()
                } else {
                    sanitized_name.clone()
                };
                output.push_str(&format!(
                    "[{}] is colored in {}\n",
                    ref_name, self.critical_color
                ));
            }

            // Completion status
            if self.show_completion && !is_milestone {
                if let Some(pct) = complete {
                    if pct > 0.0 {
                        let ref_name = if self.show_aliases {
                            alias.clone()
                        } else {
                            sanitized_name.clone()
                        };
                        output.push_str(&format!(
                            "[{}] is {}% complete\n",
                            ref_name,
                            pct.round() as i32
                        ));
                    }
                }
            }

            rendered_tasks.insert((*task_id).clone());
        }

        // Footer
        output.push_str("@endgantt\n");

        Ok(output)
    }
}

impl PlantUmlRenderer {
    /// Collect first predecessor for each task
    fn collect_predecessors(
        &self,
        task: &utf8proj_core::Task,
        map: &mut HashMap<String, String>,
    ) {
        if let Some(first_dep) = task.depends.first() {
            map.insert(task.id.clone(), first_dep.predecessor.clone());
        }
        for child in &task.children {
            self.collect_predecessors(child, map);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use utf8proj_core::{Duration, Schedule, ScheduledTask, Task};

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

        tasks.insert(
            "design".to_string(),
            ScheduledTask {
                task_id: "design".to_string(),
                start: NaiveDate::from_ymd_opt(2025, 1, 6).unwrap(),
                finish: NaiveDate::from_ymd_opt(2025, 1, 10).unwrap(),
                duration: Duration::days(5),
                assignments: vec![],
                slack: Duration::zero(),
                is_critical: true,
                early_start: NaiveDate::from_ymd_opt(2025, 1, 6).unwrap(),
                early_finish: NaiveDate::from_ymd_opt(2025, 1, 10).unwrap(),
                late_start: NaiveDate::from_ymd_opt(2025, 1, 6).unwrap(),
                late_finish: NaiveDate::from_ymd_opt(2025, 1, 10).unwrap(),
            },
        );

        tasks.insert(
            "implement".to_string(),
            ScheduledTask {
                task_id: "implement".to_string(),
                start: NaiveDate::from_ymd_opt(2025, 1, 13).unwrap(),
                finish: NaiveDate::from_ymd_opt(2025, 1, 24).unwrap(),
                duration: Duration::days(10),
                assignments: vec![],
                slack: Duration::zero(),
                is_critical: true,
                early_start: NaiveDate::from_ymd_opt(2025, 1, 13).unwrap(),
                early_finish: NaiveDate::from_ymd_opt(2025, 1, 24).unwrap(),
                late_start: NaiveDate::from_ymd_opt(2025, 1, 13).unwrap(),
                late_finish: NaiveDate::from_ymd_opt(2025, 1, 24).unwrap(),
            },
        );

        tasks.insert(
            "test".to_string(),
            ScheduledTask {
                task_id: "test".to_string(),
                start: NaiveDate::from_ymd_opt(2025, 1, 27).unwrap(),
                finish: NaiveDate::from_ymd_opt(2025, 1, 29).unwrap(),
                duration: Duration::days(3),
                assignments: vec![],
                slack: Duration::zero(),
                is_critical: true,
                early_start: NaiveDate::from_ymd_opt(2025, 1, 27).unwrap(),
                early_finish: NaiveDate::from_ymd_opt(2025, 1, 29).unwrap(),
                late_start: NaiveDate::from_ymd_opt(2025, 1, 27).unwrap(),
                late_finish: NaiveDate::from_ymd_opt(2025, 1, 29).unwrap(),
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
        }
    }

    #[test]
    fn plantuml_renderer_creation() {
        let renderer = PlantUmlRenderer::new();
        assert!(renderer.show_critical);
        assert!(renderer.close_weekends);
        assert_eq!(renderer.critical_color, "OrangeRed");
    }

    #[test]
    fn plantuml_renderer_with_options() {
        let renderer = PlantUmlRenderer::new()
            .no_critical()
            .include_weekends()
            .absolute_dates()
            .scale("week");

        assert!(!renderer.show_critical);
        assert!(!renderer.close_weekends);
        assert!(!renderer.use_dependencies);
        assert_eq!(renderer.scale, Some("week".to_string()));
    }

    #[test]
    fn plantuml_produces_valid_output() {
        let renderer = PlantUmlRenderer::new();
        let project = create_test_project();
        let schedule = create_test_schedule();

        let result = renderer.render(&project, &schedule);
        assert!(result.is_ok());

        let output = result.unwrap();
        assert!(output.starts_with("@startgantt\n"));
        assert!(output.ends_with("@endgantt\n"));
        assert!(output.contains("Project starts 2025-01-06"));
    }

    #[test]
    fn plantuml_includes_weekend_closure() {
        let renderer = PlantUmlRenderer::new();
        let project = create_test_project();
        let schedule = create_test_schedule();

        let output = renderer.render(&project, &schedule).unwrap();
        assert!(output.contains("saturday are closed"));
        assert!(output.contains("sunday are closed"));
    }

    #[test]
    fn plantuml_no_weekend_closure() {
        let renderer = PlantUmlRenderer::new().include_weekends();
        let project = create_test_project();
        let schedule = create_test_schedule();

        let output = renderer.render(&project, &schedule).unwrap();
        assert!(!output.contains("saturday are closed"));
        assert!(!output.contains("sunday are closed"));
    }

    #[test]
    fn plantuml_includes_critical_coloring() {
        let renderer = PlantUmlRenderer::new();
        let project = create_test_project();
        let schedule = create_test_schedule();

        let output = renderer.render(&project, &schedule).unwrap();
        assert!(output.contains("is colored in OrangeRed"));
    }

    #[test]
    fn plantuml_custom_colors() {
        let renderer = PlantUmlRenderer::new().critical_color("Red");
        let project = create_test_project();
        let schedule = create_test_schedule();

        let output = renderer.render(&project, &schedule).unwrap();
        assert!(output.contains("is colored in Red"));
    }

    #[test]
    fn plantuml_uses_dependency_syntax() {
        let renderer = PlantUmlRenderer::new();
        let project = create_test_project();
        let schedule = create_test_schedule();

        let output = renderer.render(&project, &schedule).unwrap();
        assert!(output.contains("starts at [design]'s end"));
        assert!(output.contains("starts at [implement]'s end"));
    }

    #[test]
    fn plantuml_absolute_dates_mode() {
        let renderer = PlantUmlRenderer::new().absolute_dates();
        let project = create_test_project();
        let schedule = create_test_schedule();

        let output = renderer.render(&project, &schedule).unwrap();
        assert!(!output.contains("starts at ["));
        assert!(output.contains("starts 2025-01-06"));
        assert!(output.contains("starts 2025-01-13"));
    }

    #[test]
    fn plantuml_empty_schedule_fails() {
        let renderer = PlantUmlRenderer::new();
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
    fn plantuml_sanitizes_special_chars() {
        assert_eq!(PlantUmlRenderer::sanitize_name("Task [1]"), "Task (1)");
        assert_eq!(PlantUmlRenderer::sanitize_name("Test\nTask"), "Test Task");
    }

    #[test]
    fn plantuml_makes_valid_aliases() {
        assert_eq!(PlantUmlRenderer::make_alias("task1"), "task1");
        assert_eq!(PlantUmlRenderer::make_alias("phase1.design"), "phase1_design");
        assert_eq!(PlantUmlRenderer::make_alias("task-with-dashes"), "task_with_dashes");
    }

    #[test]
    fn plantuml_milestone_detection() {
        let mut project = Project::new("Milestone Test");
        project.start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        project.tasks.push(Task::new("work").name("Work").effort(Duration::days(5)));
        project.tasks.push(Task::new("done").name("Project Complete").milestone().depends_on("work"));

        let mut tasks = HashMap::new();
        tasks.insert(
            "work".to_string(),
            ScheduledTask {
                task_id: "work".to_string(),
                start: NaiveDate::from_ymd_opt(2025, 1, 6).unwrap(),
                finish: NaiveDate::from_ymd_opt(2025, 1, 10).unwrap(),
                duration: Duration::days(5),
                assignments: vec![],
                slack: Duration::zero(),
                is_critical: true,
                early_start: NaiveDate::from_ymd_opt(2025, 1, 6).unwrap(),
                early_finish: NaiveDate::from_ymd_opt(2025, 1, 10).unwrap(),
                late_start: NaiveDate::from_ymd_opt(2025, 1, 6).unwrap(),
                late_finish: NaiveDate::from_ymd_opt(2025, 1, 10).unwrap(),
            },
        );
        tasks.insert(
            "done".to_string(),
            ScheduledTask {
                task_id: "done".to_string(),
                start: NaiveDate::from_ymd_opt(2025, 1, 13).unwrap(),
                finish: NaiveDate::from_ymd_opt(2025, 1, 13).unwrap(),
                duration: Duration::zero(),
                assignments: vec![],
                slack: Duration::zero(),
                is_critical: true,
                early_start: NaiveDate::from_ymd_opt(2025, 1, 13).unwrap(),
                early_finish: NaiveDate::from_ymd_opt(2025, 1, 13).unwrap(),
                late_start: NaiveDate::from_ymd_opt(2025, 1, 13).unwrap(),
                late_finish: NaiveDate::from_ymd_opt(2025, 1, 13).unwrap(),
            },
        );

        let schedule = Schedule {
            tasks,
            critical_path: vec!["work".to_string(), "done".to_string()],
            project_duration: Duration::days(5),
            project_end: NaiveDate::from_ymd_opt(2025, 1, 13).unwrap(),
            total_cost: None,
        };

        let renderer = PlantUmlRenderer::new();
        let output = renderer.render(&project, &schedule).unwrap();
        assert!(output.contains("happens at"));
    }

    #[test]
    fn plantuml_with_scale() {
        let renderer = PlantUmlRenderer::new().scale("week");
        let project = create_test_project();
        let schedule = create_test_schedule();

        let output = renderer.render(&project, &schedule).unwrap();
        assert!(output.contains("printscale week"));
    }

    #[test]
    fn plantuml_with_today_marker() {
        let renderer = PlantUmlRenderer::new().show_today();
        let project = create_test_project();
        let schedule = create_test_schedule();

        let output = renderer.render(&project, &schedule).unwrap();
        assert!(output.contains("today is colored in LightBlue"));
    }

    #[test]
    fn plantuml_task_aliases() {
        let renderer = PlantUmlRenderer::new();
        let project = create_test_project();
        let schedule = create_test_schedule();

        let output = renderer.render(&project, &schedule).unwrap();
        assert!(output.contains("as [design]"));
        assert!(output.contains("as [implement]"));
    }

    #[test]
    fn plantuml_no_aliases() {
        let renderer = PlantUmlRenderer::new().no_aliases();
        let project = create_test_project();
        let schedule = create_test_schedule();

        let output = renderer.render(&project, &schedule).unwrap();
        assert!(!output.contains(" as ["));
    }
}
