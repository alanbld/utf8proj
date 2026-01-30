//! Project Status Dashboard (RFC-0019)
//!
//! This module provides types for generating project status summaries.
//! Status dashboards answer the question: "How is my project doing right now?"
//!
//! # Core Concepts
//!
//! - **ProjectStatus**: Aggregated status metrics from a schedule
//! - **StatusIndicator**: On Track, At Risk, or Behind classification
//!
//! # Example
//!
//! ```rust
//! use chrono::NaiveDate;
//! use utf8proj_core::status::{ProjectStatus, StatusIndicator};
//!
//! // Create a status manually for demonstration
//! let status = ProjectStatus {
//!     project_name: "My Project".to_string(),
//!     status_date: NaiveDate::from_ymd_opt(2026, 2, 10).unwrap(),
//!     start_date: NaiveDate::from_ymd_opt(2026, 1, 6).unwrap(),
//!     baseline_finish: NaiveDate::from_ymd_opt(2026, 3, 15).unwrap(),
//!     forecast_finish: NaiveDate::from_ymd_opt(2026, 3, 13).unwrap(),
//!     overall_progress: 53,
//!     variance_days: -2,
//!     planned_value: 60,
//!     earned_value: 53,
//!     spi: 0.88,
//!     total_tasks: 24,
//!     completed_tasks: 12,
//!     in_progress_tasks: 5,
//!     not_started_tasks: 7,
//!     behind_tasks: 2,
//!     critical_path_length: 8,
//!     days_remaining: 23,
//! };
//!
//! assert_eq!(status.status_indicator(), StatusIndicator::OnTrack);
//! ```

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use crate::{Project, Schedule, TaskStatus};

// ============================================================================
// Core Types
// ============================================================================

/// Status classification for the project
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum StatusIndicator {
    /// Project is on or ahead of schedule (variance <= 0)
    OnTrack,
    /// Project is at risk (0 < variance <= 5 days)
    AtRisk,
    /// Project is behind schedule (variance > 5 days)
    Behind,
}

impl StatusIndicator {
    /// Get the display string for this indicator
    pub fn as_str(&self) -> &'static str {
        match self {
            StatusIndicator::OnTrack => "On Track",
            StatusIndicator::AtRisk => "At Risk",
            StatusIndicator::Behind => "Behind",
        }
    }
}

impl std::fmt::Display for StatusIndicator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Aggregated project status metrics
///
/// Created from a `Schedule` to provide a dashboard view of project health.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProjectStatus {
    /// Project name
    pub project_name: String,

    /// Status date (as-of date for the status)
    pub status_date: NaiveDate,

    /// Project start date
    pub start_date: NaiveDate,

    /// Baseline (planned) finish date
    pub baseline_finish: NaiveDate,

    /// Forecast (current) finish date
    pub forecast_finish: NaiveDate,

    /// Overall progress (0-100), weighted by task duration
    pub overall_progress: u8,

    /// Variance in calendar days (forecast - baseline, positive = late)
    pub variance_days: i64,

    /// Planned Value at status date (0-100)
    pub planned_value: u8,

    /// Earned Value (0-100), same as overall_progress
    pub earned_value: u8,

    /// Schedule Performance Index (EV / PV)
    pub spi: f64,

    /// Total number of tasks (leaf tasks only)
    pub total_tasks: usize,

    /// Number of completed tasks (100%)
    pub completed_tasks: usize,

    /// Number of in-progress tasks (0 < complete < 100)
    pub in_progress_tasks: usize,

    /// Number of not-started tasks (0%)
    pub not_started_tasks: usize,

    /// Number of tasks behind schedule
    pub behind_tasks: usize,

    /// Number of tasks on the critical path
    pub critical_path_length: usize,

    /// Days remaining until forecast finish
    pub days_remaining: i64,
}

impl ProjectStatus {
    /// Create a `ProjectStatus` from a project and its schedule.
    ///
    /// # Arguments
    ///
    /// * `project` - The project definition
    /// * `schedule` - The computed schedule
    /// * `status_date` - The as-of date for status calculation
    ///
    /// # Example
    ///
    /// ```ignore
    /// let status = ProjectStatus::from_schedule(&project, &schedule, today);
    /// println!("Progress: {}%", status.overall_progress);
    /// ```
    pub fn from_schedule(project: &Project, schedule: &Schedule, status_date: NaiveDate) -> Self {
        // Count task states
        let mut completed_tasks = 0usize;
        let mut in_progress_tasks = 0usize;
        let mut not_started_tasks = 0usize;
        let mut behind_tasks = 0usize;

        for scheduled_task in schedule.tasks.values() {
            match scheduled_task.status {
                TaskStatus::Complete => completed_tasks += 1,
                TaskStatus::InProgress | TaskStatus::AtRisk => {
                    in_progress_tasks += 1;
                    // Check if behind schedule
                    if scheduled_task.finish_variance_days > 0 {
                        behind_tasks += 1;
                    }
                }
                TaskStatus::NotStarted | TaskStatus::Blocked | TaskStatus::OnHold => {
                    not_started_tasks += 1;
                    // Check if behind schedule (should have started by now)
                    if scheduled_task.start_variance_days > 0 {
                        behind_tasks += 1;
                    }
                }
            }
        }

        let total_tasks = schedule.tasks.len();
        let critical_path_length = schedule.critical_path.len();

        // Calculate days remaining
        let days_remaining = (schedule.project_forecast_finish - status_date).num_days();

        Self {
            project_name: project.name.clone(),
            status_date,
            start_date: project.start,
            baseline_finish: schedule.project_baseline_finish,
            forecast_finish: schedule.project_forecast_finish,
            overall_progress: schedule.project_progress,
            variance_days: schedule.project_variance_days,
            planned_value: schedule.planned_value,
            earned_value: schedule.earned_value,
            spi: schedule.spi,
            total_tasks,
            completed_tasks,
            in_progress_tasks,
            not_started_tasks,
            behind_tasks,
            critical_path_length,
            days_remaining,
        }
    }

    /// Get the status indicator based on variance.
    ///
    /// - On Track: variance <= 0 (ahead or on schedule)
    /// - At Risk: 0 < variance <= 5 days
    /// - Behind: variance > 5 days
    pub fn status_indicator(&self) -> StatusIndicator {
        match self.variance_days {
            d if d <= 0 => StatusIndicator::OnTrack,
            d if d <= 5 => StatusIndicator::AtRisk,
            _ => StatusIndicator::Behind,
        }
    }

    /// Get a formatted variance string (e.g., "+2 days", "-3 days", "on schedule")
    pub fn variance_string(&self) -> String {
        match self.variance_days {
            0 => "on schedule".to_string(),
            d if d > 0 => format!("+{} days late", d),
            d => format!("{} days ahead", d.abs()),
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Duration, ScheduledTask};
    use std::collections::HashMap;

    fn date(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).unwrap()
    }

    fn create_test_project() -> Project {
        let mut project = Project::new("Test Project");
        project.start = date(2026, 1, 6);
        project
    }

    fn create_scheduled_task(
        task_id: &str,
        start: NaiveDate,
        finish: NaiveDate,
        status: TaskStatus,
        finish_variance_days: i64,
    ) -> ScheduledTask {
        ScheduledTask {
            task_id: task_id.to_string(),
            start,
            finish,
            duration: Duration::days((finish - start).num_days()),
            assignments: Vec::new(),
            slack: Duration::zero(),
            is_critical: false,
            early_start: start,
            early_finish: finish,
            late_start: start,
            late_finish: finish,
            forecast_start: start,
            forecast_finish: finish,
            remaining_duration: Duration::zero(),
            percent_complete: match status {
                TaskStatus::Complete => 100,
                TaskStatus::InProgress | TaskStatus::AtRisk => 50,
                TaskStatus::NotStarted | TaskStatus::Blocked | TaskStatus::OnHold => 0,
            },
            status,
            baseline_start: start,
            baseline_finish: finish,
            start_variance_days: 0,
            finish_variance_days,
            cost_range: None,
            has_abstract_assignments: false,
        }
    }

    #[test]
    fn test_status_indicator_on_track() {
        let status = ProjectStatus {
            project_name: "Test".to_string(),
            status_date: date(2026, 2, 10),
            start_date: date(2026, 1, 6),
            baseline_finish: date(2026, 3, 15),
            forecast_finish: date(2026, 3, 13),
            overall_progress: 53,
            variance_days: -2,
            planned_value: 60,
            earned_value: 53,
            spi: 0.88,
            total_tasks: 24,
            completed_tasks: 12,
            in_progress_tasks: 5,
            not_started_tasks: 7,
            behind_tasks: 2,
            critical_path_length: 8,
            days_remaining: 23,
        };

        assert_eq!(status.status_indicator(), StatusIndicator::OnTrack);
    }

    #[test]
    fn test_status_indicator_at_risk() {
        let status = ProjectStatus {
            project_name: "Test".to_string(),
            status_date: date(2026, 2, 10),
            start_date: date(2026, 1, 6),
            baseline_finish: date(2026, 3, 15),
            forecast_finish: date(2026, 3, 18),
            overall_progress: 45,
            variance_days: 3,
            planned_value: 60,
            earned_value: 45,
            spi: 0.75,
            total_tasks: 24,
            completed_tasks: 10,
            in_progress_tasks: 6,
            not_started_tasks: 8,
            behind_tasks: 4,
            critical_path_length: 8,
            days_remaining: 26,
        };

        assert_eq!(status.status_indicator(), StatusIndicator::AtRisk);
    }

    #[test]
    fn test_status_indicator_behind() {
        let status = ProjectStatus {
            project_name: "Test".to_string(),
            status_date: date(2026, 2, 10),
            start_date: date(2026, 1, 6),
            baseline_finish: date(2026, 3, 15),
            forecast_finish: date(2026, 3, 25),
            overall_progress: 35,
            variance_days: 10,
            planned_value: 60,
            earned_value: 35,
            spi: 0.58,
            total_tasks: 24,
            completed_tasks: 8,
            in_progress_tasks: 5,
            not_started_tasks: 11,
            behind_tasks: 8,
            critical_path_length: 8,
            days_remaining: 33,
        };

        assert_eq!(status.status_indicator(), StatusIndicator::Behind);
    }

    #[test]
    fn test_status_indicator_boundary_at_risk() {
        // Exactly 5 days variance should be At Risk
        let status = ProjectStatus {
            project_name: "Test".to_string(),
            status_date: date(2026, 2, 10),
            start_date: date(2026, 1, 6),
            baseline_finish: date(2026, 3, 15),
            forecast_finish: date(2026, 3, 20),
            overall_progress: 50,
            variance_days: 5,
            planned_value: 60,
            earned_value: 50,
            spi: 0.83,
            total_tasks: 10,
            completed_tasks: 5,
            in_progress_tasks: 3,
            not_started_tasks: 2,
            behind_tasks: 2,
            critical_path_length: 5,
            days_remaining: 28,
        };

        assert_eq!(status.status_indicator(), StatusIndicator::AtRisk);
    }

    #[test]
    fn test_status_indicator_boundary_behind() {
        // 6 days variance should be Behind
        let status = ProjectStatus {
            project_name: "Test".to_string(),
            status_date: date(2026, 2, 10),
            start_date: date(2026, 1, 6),
            baseline_finish: date(2026, 3, 15),
            forecast_finish: date(2026, 3, 21),
            overall_progress: 48,
            variance_days: 6,
            planned_value: 60,
            earned_value: 48,
            spi: 0.80,
            total_tasks: 10,
            completed_tasks: 5,
            in_progress_tasks: 3,
            not_started_tasks: 2,
            behind_tasks: 3,
            critical_path_length: 5,
            days_remaining: 29,
        };

        assert_eq!(status.status_indicator(), StatusIndicator::Behind);
    }

    #[test]
    fn test_status_indicator_boundary_on_track() {
        // Exactly 0 days variance should be On Track
        let status = ProjectStatus {
            project_name: "Test".to_string(),
            status_date: date(2026, 2, 10),
            start_date: date(2026, 1, 6),
            baseline_finish: date(2026, 3, 15),
            forecast_finish: date(2026, 3, 15),
            overall_progress: 60,
            variance_days: 0,
            planned_value: 60,
            earned_value: 60,
            spi: 1.0,
            total_tasks: 10,
            completed_tasks: 6,
            in_progress_tasks: 2,
            not_started_tasks: 2,
            behind_tasks: 0,
            critical_path_length: 5,
            days_remaining: 23,
        };

        assert_eq!(status.status_indicator(), StatusIndicator::OnTrack);
    }

    #[test]
    fn test_variance_string_ahead() {
        let status = ProjectStatus {
            project_name: "Test".to_string(),
            status_date: date(2026, 2, 10),
            start_date: date(2026, 1, 6),
            baseline_finish: date(2026, 3, 15),
            forecast_finish: date(2026, 3, 12),
            overall_progress: 55,
            variance_days: -3,
            planned_value: 50,
            earned_value: 55,
            spi: 1.1,
            total_tasks: 10,
            completed_tasks: 5,
            in_progress_tasks: 3,
            not_started_tasks: 2,
            behind_tasks: 0,
            critical_path_length: 5,
            days_remaining: 20,
        };

        assert_eq!(status.variance_string(), "3 days ahead");
    }

    #[test]
    fn test_variance_string_late() {
        let status = ProjectStatus {
            project_name: "Test".to_string(),
            status_date: date(2026, 2, 10),
            start_date: date(2026, 1, 6),
            baseline_finish: date(2026, 3, 15),
            forecast_finish: date(2026, 3, 17),
            overall_progress: 45,
            variance_days: 2,
            planned_value: 50,
            earned_value: 45,
            spi: 0.9,
            total_tasks: 10,
            completed_tasks: 4,
            in_progress_tasks: 4,
            not_started_tasks: 2,
            behind_tasks: 2,
            critical_path_length: 5,
            days_remaining: 25,
        };

        assert_eq!(status.variance_string(), "+2 days late");
    }

    #[test]
    fn test_variance_string_on_schedule() {
        let status = ProjectStatus {
            project_name: "Test".to_string(),
            status_date: date(2026, 2, 10),
            start_date: date(2026, 1, 6),
            baseline_finish: date(2026, 3, 15),
            forecast_finish: date(2026, 3, 15),
            overall_progress: 50,
            variance_days: 0,
            planned_value: 50,
            earned_value: 50,
            spi: 1.0,
            total_tasks: 10,
            completed_tasks: 5,
            in_progress_tasks: 3,
            not_started_tasks: 2,
            behind_tasks: 0,
            critical_path_length: 5,
            days_remaining: 23,
        };

        assert_eq!(status.variance_string(), "on schedule");
    }

    #[test]
    fn test_from_schedule_empty_project() {
        let project = create_test_project();
        let schedule = Schedule {
            tasks: HashMap::new(),
            critical_path: Vec::new(),
            project_duration: Duration::zero(),
            project_end: date(2026, 1, 6),
            total_cost: None,
            total_cost_range: None,
            project_progress: 0,
            project_baseline_finish: date(2026, 1, 6),
            project_forecast_finish: date(2026, 1, 6),
            project_variance_days: 0,
            planned_value: 0,
            earned_value: 0,
            spi: 1.0,
        };

        let status = ProjectStatus::from_schedule(&project, &schedule, date(2026, 2, 10));

        assert_eq!(status.project_name, "Test Project");
        assert_eq!(status.total_tasks, 0);
        assert_eq!(status.completed_tasks, 0);
        assert_eq!(status.in_progress_tasks, 0);
        assert_eq!(status.not_started_tasks, 0);
        assert_eq!(status.behind_tasks, 0);
        assert_eq!(status.critical_path_length, 0);
    }

    #[test]
    fn test_from_schedule_task_counts() {
        let project = create_test_project();
        let mut tasks = HashMap::new();

        // Add complete task
        tasks.insert(
            "task1".to_string(),
            create_scheduled_task("task1", date(2026, 1, 6), date(2026, 1, 10), TaskStatus::Complete, 0),
        );

        // Add in-progress task (on schedule)
        tasks.insert(
            "task2".to_string(),
            create_scheduled_task("task2", date(2026, 1, 11), date(2026, 1, 15), TaskStatus::InProgress, 0),
        );

        // Add in-progress task (behind schedule)
        tasks.insert(
            "task3".to_string(),
            create_scheduled_task("task3", date(2026, 1, 11), date(2026, 1, 17), TaskStatus::InProgress, 2),
        );

        // Add not-started task
        tasks.insert(
            "task4".to_string(),
            create_scheduled_task("task4", date(2026, 1, 18), date(2026, 1, 22), TaskStatus::NotStarted, 0),
        );

        let schedule = Schedule {
            tasks,
            critical_path: vec!["task1".to_string(), "task2".to_string()],
            project_duration: Duration::days(17),
            project_end: date(2026, 1, 22),
            total_cost: None,
            total_cost_range: None,
            project_progress: 40,
            project_baseline_finish: date(2026, 1, 22),
            project_forecast_finish: date(2026, 1, 22),
            project_variance_days: 0,
            planned_value: 50,
            earned_value: 40,
            spi: 0.8,
        };

        let status = ProjectStatus::from_schedule(&project, &schedule, date(2026, 1, 13));

        assert_eq!(status.total_tasks, 4);
        assert_eq!(status.completed_tasks, 1);
        assert_eq!(status.in_progress_tasks, 2);
        assert_eq!(status.not_started_tasks, 1);
        assert_eq!(status.behind_tasks, 1); // One in-progress task is behind
        assert_eq!(status.critical_path_length, 2);
        assert_eq!(status.overall_progress, 40);
        assert_eq!(status.spi, 0.8);
    }

    #[test]
    fn test_from_schedule_all_complete() {
        let project = create_test_project();
        let mut tasks = HashMap::new();

        tasks.insert(
            "task1".to_string(),
            create_scheduled_task("task1", date(2026, 1, 6), date(2026, 1, 10), TaskStatus::Complete, 0),
        );
        tasks.insert(
            "task2".to_string(),
            create_scheduled_task("task2", date(2026, 1, 11), date(2026, 1, 15), TaskStatus::Complete, 0),
        );

        let schedule = Schedule {
            tasks,
            critical_path: vec!["task1".to_string(), "task2".to_string()],
            project_duration: Duration::days(10),
            project_end: date(2026, 1, 15),
            total_cost: None,
            total_cost_range: None,
            project_progress: 100,
            project_baseline_finish: date(2026, 1, 15),
            project_forecast_finish: date(2026, 1, 15),
            project_variance_days: 0,
            planned_value: 100,
            earned_value: 100,
            spi: 1.0,
        };

        let status = ProjectStatus::from_schedule(&project, &schedule, date(2026, 1, 16));

        assert_eq!(status.total_tasks, 2);
        assert_eq!(status.completed_tasks, 2);
        assert_eq!(status.in_progress_tasks, 0);
        assert_eq!(status.not_started_tasks, 0);
        assert_eq!(status.overall_progress, 100);
    }

    #[test]
    fn test_from_schedule_days_remaining() {
        let project = create_test_project();
        let mut tasks = HashMap::new();

        tasks.insert(
            "task1".to_string(),
            create_scheduled_task("task1", date(2026, 1, 6), date(2026, 1, 20), TaskStatus::InProgress, 0),
        );

        let schedule = Schedule {
            tasks,
            critical_path: vec!["task1".to_string()],
            project_duration: Duration::days(15),
            project_end: date(2026, 1, 20),
            total_cost: None,
            total_cost_range: None,
            project_progress: 50,
            project_baseline_finish: date(2026, 1, 20),
            project_forecast_finish: date(2026, 1, 22),
            project_variance_days: 2,
            planned_value: 50,
            earned_value: 50,
            spi: 1.0,
        };

        let status = ProjectStatus::from_schedule(&project, &schedule, date(2026, 1, 10));

        // 12 days from Jan 10 to Jan 22 (forecast finish)
        assert_eq!(status.days_remaining, 12);
    }

    #[test]
    fn test_status_indicator_display() {
        assert_eq!(StatusIndicator::OnTrack.as_str(), "On Track");
        assert_eq!(StatusIndicator::AtRisk.as_str(), "At Risk");
        assert_eq!(StatusIndicator::Behind.as_str(), "Behind");

        assert_eq!(format!("{}", StatusIndicator::OnTrack), "On Track");
    }

    #[test]
    fn test_earned_value_metrics() {
        let project = create_test_project();
        let mut tasks = HashMap::new();

        tasks.insert(
            "task1".to_string(),
            create_scheduled_task("task1", date(2026, 1, 6), date(2026, 1, 15), TaskStatus::InProgress, 0),
        );

        let schedule = Schedule {
            tasks,
            critical_path: vec!["task1".to_string()],
            project_duration: Duration::days(10),
            project_end: date(2026, 1, 15),
            total_cost: None,
            total_cost_range: None,
            project_progress: 53,
            project_baseline_finish: date(2026, 1, 15),
            project_forecast_finish: date(2026, 1, 17),
            project_variance_days: 2,
            planned_value: 60,
            earned_value: 53,
            spi: 0.88,
        };

        let status = ProjectStatus::from_schedule(&project, &schedule, date(2026, 1, 10));

        assert_eq!(status.planned_value, 60);
        assert_eq!(status.earned_value, 53);
        assert!((status.spi - 0.88).abs() < 0.001);
    }
}
