//! # utf8proj-core
//!
//! Core domain model and traits for the utf8proj scheduling engine.
//!
//! This crate provides:
//! - Domain types: `Project`, `Task`, `Resource`, `Calendar`, `Schedule`
//! - Core traits: `Scheduler`, `WhatIfAnalysis`, `Renderer`
//! - Error types and result aliases
//!
//! ## Example
//!
//! ```rust
//! use utf8proj_core::{Project, Task, Resource, Duration};
//!
//! let mut project = Project::new("My Project");
//! project.tasks.push(
//!     Task::new("design")
//!         .effort(Duration::days(5))
//!         .assign("dev")
//! );
//! project.tasks.push(
//!     Task::new("implement")
//!         .effort(Duration::days(10))
//!         .depends_on("design")
//!         .assign("dev")
//! );
//! project.resources.push(Resource::new("dev").capacity(1.0));
//! ```

use chrono::{Datelike, NaiveDate};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

// ============================================================================
// Type Aliases
// ============================================================================

/// Unique identifier for a task
pub type TaskId = String;

/// Unique identifier for a resource
pub type ResourceId = String;

/// Unique identifier for a calendar
pub type CalendarId = String;

/// Duration in working time
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Duration {
    /// Number of minutes
    pub minutes: i64,
}

impl Duration {
    pub const fn zero() -> Self {
        Self { minutes: 0 }
    }

    pub const fn minutes(m: i64) -> Self {
        Self { minutes: m }
    }

    pub const fn hours(h: i64) -> Self {
        Self { minutes: h * 60 }
    }

    pub const fn days(d: i64) -> Self {
        Self { minutes: d * 8 * 60 } // 8-hour workday
    }

    pub const fn weeks(w: i64) -> Self {
        Self { minutes: w * 5 * 8 * 60 } // 5-day workweek
    }

    pub fn as_days(&self) -> f64 {
        self.minutes as f64 / (8.0 * 60.0)
    }

    pub fn as_hours(&self) -> f64 {
        self.minutes as f64 / 60.0
    }
}

impl std::ops::Add for Duration {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self { minutes: self.minutes + rhs.minutes }
    }
}

impl std::ops::Sub for Duration {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Self { minutes: self.minutes - rhs.minutes }
    }
}

/// Monetary amount with currency
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Money {
    pub amount: Decimal,
    pub currency: String,
}

impl Money {
    pub fn new(amount: impl Into<Decimal>, currency: impl Into<String>) -> Self {
        Self {
            amount: amount.into(),
            currency: currency.into(),
        }
    }
}

// ============================================================================
// Project
// ============================================================================

/// A complete project definition
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Project {
    /// Unique identifier
    pub id: String,
    /// Human-readable name
    pub name: String,
    /// Project start date
    pub start: NaiveDate,
    /// Project end date (optional, can be computed)
    pub end: Option<NaiveDate>,
    /// Default calendar for the project
    pub calendar: CalendarId,
    /// Currency for cost calculations
    pub currency: String,
    /// All tasks in the project (may be hierarchical)
    pub tasks: Vec<Task>,
    /// All resources available to the project
    pub resources: Vec<Resource>,
    /// Calendar definitions
    pub calendars: Vec<Calendar>,
    /// Scenario definitions (for what-if analysis)
    pub scenarios: Vec<Scenario>,
    /// Custom attributes (timezone, etc.)
    pub attributes: HashMap<String, String>,
}

impl Project {
    /// Create a new project with the given name
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: String::new(),
            name: name.into(),
            start: NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
            end: None,
            calendar: "default".into(),
            currency: "USD".into(),
            tasks: Vec::new(),
            resources: Vec::new(),
            calendars: vec![Calendar::default()],
            scenarios: Vec::new(),
            attributes: HashMap::new(),
        }
    }

    /// Get a task by ID (searches recursively)
    pub fn get_task(&self, id: &str) -> Option<&Task> {
        fn find_task<'a>(tasks: &'a [Task], id: &str) -> Option<&'a Task> {
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
        find_task(&self.tasks, id)
    }

    /// Get a resource by ID
    pub fn get_resource(&self, id: &str) -> Option<&Resource> {
        self.resources.iter().find(|r| r.id == id)
    }

    /// Get all leaf tasks (tasks without children)
    pub fn leaf_tasks(&self) -> Vec<&Task> {
        fn collect_leaves<'a>(tasks: &'a [Task], result: &mut Vec<&'a Task>) {
            for task in tasks {
                if task.children.is_empty() {
                    result.push(task);
                } else {
                    collect_leaves(&task.children, result);
                }
            }
        }
        let mut leaves = Vec::new();
        collect_leaves(&self.tasks, &mut leaves);
        leaves
    }
}

// ============================================================================
// Task
// ============================================================================

/// A schedulable unit of work
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Task {
    /// Unique identifier
    pub id: TaskId,
    /// Human-readable name
    pub name: String,
    /// Work effort required (person-time)
    pub effort: Option<Duration>,
    /// Calendar duration (overrides effort-based calculation)
    pub duration: Option<Duration>,
    /// Task dependencies
    pub depends: Vec<Dependency>,
    /// Resource assignments
    pub assigned: Vec<ResourceRef>,
    /// Scheduling priority (higher = scheduled first)
    pub priority: u32,
    /// Scheduling constraints
    pub constraints: Vec<TaskConstraint>,
    /// Is this a milestone (zero duration)?
    pub milestone: bool,
    /// Child tasks (WBS hierarchy)
    pub children: Vec<Task>,
    /// Completion percentage (for tracking)
    pub complete: Option<f32>,
    /// Actual start date (when work actually began)
    pub actual_start: Option<NaiveDate>,
    /// Actual finish date (when work actually completed)
    pub actual_finish: Option<NaiveDate>,
    /// Task status for progress tracking
    pub status: Option<TaskStatus>,
    /// Custom attributes
    pub attributes: HashMap<String, String>,
}

impl Task {
    /// Create a new task with the given ID
    pub fn new(id: impl Into<String>) -> Self {
        let id = id.into();
        Self {
            name: id.clone(),
            id,
            effort: None,
            duration: None,
            depends: Vec::new(),
            assigned: Vec::new(),
            priority: 500,
            constraints: Vec::new(),
            milestone: false,
            children: Vec::new(),
            complete: None,
            actual_start: None,
            actual_finish: None,
            status: None,
            attributes: HashMap::new(),
        }
    }

    /// Set the task name
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    /// Set the effort
    pub fn effort(mut self, effort: Duration) -> Self {
        self.effort = Some(effort);
        self
    }

    /// Set the duration
    pub fn duration(mut self, duration: Duration) -> Self {
        self.duration = Some(duration);
        self
    }

    /// Add a dependency (FinishToStart by default)
    pub fn depends_on(mut self, predecessor: impl Into<String>) -> Self {
        self.depends.push(Dependency {
            predecessor: predecessor.into(),
            dep_type: DependencyType::FinishToStart,
            lag: None,
        });
        self
    }

    /// Add a dependency with full control over type and lag
    pub fn with_dependency(mut self, dep: Dependency) -> Self {
        self.depends.push(dep);
        self
    }

    /// Assign a resource
    pub fn assign(mut self, resource: impl Into<String>) -> Self {
        self.assigned.push(ResourceRef {
            resource_id: resource.into(),
            units: 1.0,
        });
        self
    }

    /// Assign a resource with specific allocation units
    ///
    /// Units represent allocation percentage: 1.0 = 100%, 0.5 = 50%, etc.
    /// This affects effort-driven duration calculation:
    ///   Duration = Effort / Total_Units
    pub fn assign_with_units(mut self, resource: impl Into<String>, units: f32) -> Self {
        self.assigned.push(ResourceRef {
            resource_id: resource.into(),
            units,
        });
        self
    }

    /// Set priority
    pub fn priority(mut self, priority: u32) -> Self {
        self.priority = priority;
        self
    }

    /// Mark as milestone
    pub fn milestone(mut self) -> Self {
        self.milestone = true;
        self.duration = Some(Duration::zero());
        self
    }

    /// Add a child task
    pub fn child(mut self, child: Task) -> Self {
        self.children.push(child);
        self
    }

    /// Check if this is a summary task (has children)
    pub fn is_summary(&self) -> bool {
        !self.children.is_empty()
    }

    // ========================================================================
    // Progress Tracking Methods
    // ========================================================================

    /// Calculate remaining duration based on completion percentage.
    /// Uses linear interpolation: remaining = original × (1 - complete/100)
    pub fn remaining_duration(&self) -> Duration {
        let original = self.duration.or(self.effort).unwrap_or(Duration::zero());
        let pct = self.effective_percent_complete() as f64;
        let remaining_minutes = (original.minutes as f64 * (1.0 - pct / 100.0)).round() as i64;
        Duration::minutes(remaining_minutes.max(0))
    }

    /// Get effective completion percentage as u8 (0-100).
    /// Returns 0 if not set, clamped to 0-100 range.
    pub fn effective_percent_complete(&self) -> u8 {
        self.complete
            .map(|c| c.clamp(0.0, 100.0) as u8)
            .unwrap_or(0)
    }

    /// Derive task status from actual dates and completion.
    /// Returns explicit status if set, otherwise derives from data.
    pub fn derived_status(&self) -> TaskStatus {
        // Use explicit status if set
        if let Some(ref status) = self.status {
            return status.clone();
        }

        // Derive from actual data
        let pct = self.effective_percent_complete();
        if pct >= 100 || self.actual_finish.is_some() {
            TaskStatus::Complete
        } else if pct > 0 || self.actual_start.is_some() {
            TaskStatus::InProgress
        } else {
            TaskStatus::NotStarted
        }
    }

    /// Set the completion percentage (builder pattern)
    pub fn complete(mut self, pct: f32) -> Self {
        self.complete = Some(pct);
        self
    }

    /// Check if this task is a container (has children)
    pub fn is_container(&self) -> bool {
        !self.children.is_empty()
    }

    /// Calculate container progress as weighted average of children by duration.
    /// Returns None if not a container or if no children have duration.
    /// Formula: Σ(child.percent_complete × child.duration) / Σ(child.duration)
    pub fn container_progress(&self) -> Option<u8> {
        if self.children.is_empty() {
            return None;
        }

        let (total_weighted, total_duration) = self.calculate_weighted_progress();

        if total_duration == 0 {
            return None;
        }

        Some((total_weighted as f64 / total_duration as f64).round() as u8)
    }

    /// Helper to recursively calculate weighted progress from all descendants.
    /// Returns (weighted_sum, total_duration_minutes)
    fn calculate_weighted_progress(&self) -> (i64, i64) {
        let mut total_weighted: i64 = 0;
        let mut total_duration: i64 = 0;

        for child in &self.children {
            if child.is_container() {
                // Recursively get progress from nested containers
                let (child_weighted, child_duration) = child.calculate_weighted_progress();
                total_weighted += child_weighted;
                total_duration += child_duration;
            } else {
                // Leaf task - use its duration and progress
                let duration = child.duration.or(child.effort).unwrap_or(Duration::zero());
                let duration_mins = duration.minutes;
                let pct = child.effective_percent_complete() as i64;

                total_weighted += pct * duration_mins;
                total_duration += duration_mins;
            }
        }

        (total_weighted, total_duration)
    }

    /// Get the effective progress for this task, considering container rollup.
    /// For containers: returns derived progress from children (unless manually overridden).
    /// For leaf tasks: returns the explicit completion percentage.
    pub fn effective_progress(&self) -> u8 {
        // If manual override is set, use it
        if let Some(pct) = self.complete {
            return pct.clamp(0.0, 100.0) as u8;
        }

        // For containers, derive from children
        if let Some(derived) = self.container_progress() {
            return derived;
        }

        // Default to 0
        0
    }

    /// Check if container progress significantly differs from manual override.
    /// Returns Some((manual, derived)) if mismatch > threshold, None otherwise.
    pub fn progress_mismatch(&self, threshold: u8) -> Option<(u8, u8)> {
        if !self.is_container() {
            return None;
        }

        let manual = self.complete.map(|c| c.clamp(0.0, 100.0) as u8)?;
        let derived = self.container_progress()?;

        let diff = (manual as i16 - derived as i16).unsigned_abs() as u8;
        if diff > threshold {
            Some((manual, derived))
        } else {
            None
        }
    }

    /// Set the actual start date (builder pattern)
    pub fn actual_start(mut self, date: NaiveDate) -> Self {
        self.actual_start = Some(date);
        self
    }

    /// Set the actual finish date (builder pattern)
    pub fn actual_finish(mut self, date: NaiveDate) -> Self {
        self.actual_finish = Some(date);
        self
    }

    /// Set the task status (builder pattern)
    pub fn with_status(mut self, status: TaskStatus) -> Self {
        self.status = Some(status);
        self
    }
}

/// Task dependency with type and lag
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Dependency {
    /// ID of the predecessor task
    pub predecessor: TaskId,
    /// Type of dependency
    pub dep_type: DependencyType,
    /// Lag time (positive) or lead time (negative)
    pub lag: Option<Duration>,
}

/// Types of task dependencies
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum DependencyType {
    /// Finish-to-Start: successor starts after predecessor finishes
    #[default]
    FinishToStart,
    /// Start-to-Start: successor starts when predecessor starts
    StartToStart,
    /// Finish-to-Finish: successor finishes when predecessor finishes
    FinishToFinish,
    /// Start-to-Finish: successor finishes when predecessor starts
    StartToFinish,
}

/// Task status for progress tracking
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskStatus {
    #[default]
    NotStarted,
    InProgress,
    Complete,
    Blocked,
    AtRisk,
    OnHold,
}

impl std::fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TaskStatus::NotStarted => write!(f, "Not Started"),
            TaskStatus::InProgress => write!(f, "In Progress"),
            TaskStatus::Complete => write!(f, "Complete"),
            TaskStatus::Blocked => write!(f, "Blocked"),
            TaskStatus::AtRisk => write!(f, "At Risk"),
            TaskStatus::OnHold => write!(f, "On Hold"),
        }
    }
}

/// Reference to a resource with allocation units
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ResourceRef {
    /// ID of the resource
    pub resource_id: ResourceId,
    /// Allocation units (1.0 = 100%)
    pub units: f32,
}

/// Constraint on task scheduling
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum TaskConstraint {
    /// Task must start on this date
    MustStartOn(NaiveDate),
    /// Task must finish on this date
    MustFinishOn(NaiveDate),
    /// Task cannot start before this date
    StartNoEarlierThan(NaiveDate),
    /// Task must start by this date
    StartNoLaterThan(NaiveDate),
    /// Task cannot finish before this date
    FinishNoEarlierThan(NaiveDate),
    /// Task must finish by this date
    FinishNoLaterThan(NaiveDate),
}

// ============================================================================
// Resource
// ============================================================================

/// A person or equipment that can be assigned to tasks
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Resource {
    /// Unique identifier
    pub id: ResourceId,
    /// Human-readable name
    pub name: String,
    /// Cost rate (per time unit)
    pub rate: Option<Money>,
    /// Capacity (1.0 = full time, 0.5 = half time)
    pub capacity: f32,
    /// Custom calendar (overrides project default)
    pub calendar: Option<CalendarId>,
    /// Efficiency factor (default 1.0)
    pub efficiency: f32,
    /// Custom attributes
    pub attributes: HashMap<String, String>,
}

impl Resource {
    /// Create a new resource with the given ID
    pub fn new(id: impl Into<String>) -> Self {
        let id = id.into();
        Self {
            name: id.clone(),
            id,
            rate: None,
            capacity: 1.0,
            calendar: None,
            efficiency: 1.0,
            attributes: HashMap::new(),
        }
    }

    /// Set the resource name
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    /// Set the capacity
    pub fn capacity(mut self, capacity: f32) -> Self {
        self.capacity = capacity;
        self
    }

    /// Set the cost rate
    pub fn rate(mut self, rate: Money) -> Self {
        self.rate = Some(rate);
        self
    }

    /// Set the efficiency factor
    pub fn efficiency(mut self, efficiency: f32) -> Self {
        self.efficiency = efficiency;
        self
    }
}

// ============================================================================
// Calendar
// ============================================================================

/// Working time definitions
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Calendar {
    /// Unique identifier
    pub id: CalendarId,
    /// Human-readable name
    pub name: String,
    /// Working hours per day
    pub working_hours: Vec<TimeRange>,
    /// Working days (0 = Sunday, 6 = Saturday)
    pub working_days: Vec<u8>,
    /// Holiday dates
    pub holidays: Vec<Holiday>,
    /// Exceptions (override working hours for specific dates)
    pub exceptions: Vec<CalendarException>,
}

impl Default for Calendar {
    fn default() -> Self {
        Self {
            id: "default".into(),
            name: "Standard".into(),
            working_hours: vec![
                TimeRange { start: 9 * 60, end: 12 * 60 },
                TimeRange { start: 13 * 60, end: 17 * 60 },
            ],
            working_days: vec![1, 2, 3, 4, 5], // Mon-Fri
            holidays: Vec::new(),
            exceptions: Vec::new(),
        }
    }
}

impl Calendar {
    /// Calculate working hours per day
    pub fn hours_per_day(&self) -> f64 {
        self.working_hours.iter().map(|r| r.duration_hours()).sum()
    }

    /// Check if a date is a working day
    pub fn is_working_day(&self, date: NaiveDate) -> bool {
        let weekday = date.weekday().num_days_from_sunday() as u8;
        if !self.working_days.contains(&weekday) {
            return false;
        }
        if self.holidays.iter().any(|h| h.contains(date)) {
            return false;
        }
        true
    }
}

/// Time range within a day (in minutes from midnight)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TimeRange {
    pub start: u16, // Minutes from midnight
    pub end: u16,
}

impl TimeRange {
    pub fn duration_hours(&self) -> f64 {
        (self.end - self.start) as f64 / 60.0
    }
}

/// Holiday definition
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Holiday {
    pub name: String,
    pub start: NaiveDate,
    pub end: NaiveDate,
}

impl Holiday {
    pub fn contains(&self, date: NaiveDate) -> bool {
        date >= self.start && date <= self.end
    }
}

/// Calendar exception (override for specific dates)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CalendarException {
    pub date: NaiveDate,
    pub working_hours: Option<Vec<TimeRange>>, // None = non-working
}

// ============================================================================
// Scenario
// ============================================================================

/// Alternative scenario for what-if analysis
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Scenario {
    pub id: String,
    pub name: String,
    pub parent: Option<String>,
    pub overrides: Vec<ScenarioOverride>,
}

/// Override for a scenario
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ScenarioOverride {
    TaskEffort { task_id: TaskId, effort: Duration },
    TaskDuration { task_id: TaskId, duration: Duration },
    ResourceCapacity { resource_id: ResourceId, capacity: f32 },
}

// ============================================================================
// Schedule (Result)
// ============================================================================

/// The result of scheduling a project
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Schedule {
    /// Scheduled tasks indexed by ID
    pub tasks: HashMap<TaskId, ScheduledTask>,
    /// Tasks on the critical path
    pub critical_path: Vec<TaskId>,
    /// Total project duration
    pub project_duration: Duration,
    /// Project end date
    pub project_end: NaiveDate,
    /// Total project cost
    pub total_cost: Option<Money>,
}

/// A task with computed schedule information
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ScheduledTask {
    /// Task ID
    pub task_id: TaskId,
    /// Scheduled start date
    pub start: NaiveDate,
    /// Scheduled finish date
    pub finish: NaiveDate,
    /// Actual duration
    pub duration: Duration,
    /// Resource assignments with time periods
    pub assignments: Vec<Assignment>,
    /// Slack/float time
    pub slack: Duration,
    /// Is this task on the critical path?
    pub is_critical: bool,
    /// Early start date
    pub early_start: NaiveDate,
    /// Early finish date
    pub early_finish: NaiveDate,
    /// Late start date
    pub late_start: NaiveDate,
    /// Late finish date
    pub late_finish: NaiveDate,

    // ========================================================================
    // Progress Tracking Fields
    // ========================================================================

    /// Forecast start (actual_start if available, otherwise planned start)
    pub forecast_start: NaiveDate,
    /// Forecast finish date (calculated based on progress)
    pub forecast_finish: NaiveDate,
    /// Remaining duration based on progress
    pub remaining_duration: Duration,
    /// Completion percentage (0-100)
    pub percent_complete: u8,
    /// Current task status
    pub status: TaskStatus,
}

impl ScheduledTask {
    /// Create a test ScheduledTask with default progress tracking fields.
    /// Useful for unit tests that don't need progress data.
    #[cfg(test)]
    pub fn test_new(
        task_id: impl Into<String>,
        start: NaiveDate,
        finish: NaiveDate,
        duration: Duration,
        slack: Duration,
        is_critical: bool,
    ) -> Self {
        let task_id = task_id.into();
        Self {
            task_id,
            start,
            finish,
            duration,
            assignments: Vec::new(),
            slack,
            is_critical,
            early_start: start,
            early_finish: finish,
            late_start: start,
            late_finish: finish,
            forecast_start: start,
            forecast_finish: finish,
            remaining_duration: duration,
            percent_complete: 0,
            status: TaskStatus::NotStarted,
        }
    }
}

/// Resource assignment for a specific period
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Assignment {
    pub resource_id: ResourceId,
    pub start: NaiveDate,
    pub finish: NaiveDate,
    pub units: f32,
    pub cost: Option<Money>,
}

// ============================================================================
// Traits
// ============================================================================

/// Core scheduling abstraction
pub trait Scheduler: Send + Sync {
    /// Compute a schedule for the given project
    fn schedule(&self, project: &Project) -> Result<Schedule, ScheduleError>;

    /// Check if a schedule is feasible without computing it
    fn is_feasible(&self, project: &Project) -> FeasibilityResult;

    /// Explain why a particular scheduling decision was made
    fn explain(&self, project: &Project, task: &TaskId) -> Explanation;
}

/// What-if analysis capabilities (typically BDD-powered)
pub trait WhatIfAnalysis {
    /// Analyze impact of a constraint change
    fn what_if(&self, project: &Project, change: &Constraint) -> WhatIfReport;

    /// Count valid schedules under current constraints
    fn count_solutions(&self, project: &Project) -> num_bigint::BigUint;

    /// Find all critical constraints
    fn critical_constraints(&self, project: &Project) -> Vec<Constraint>;
}

/// Output rendering
pub trait Renderer {
    type Output;

    /// Render a schedule to the output format
    fn render(&self, project: &Project, schedule: &Schedule) -> Result<Self::Output, RenderError>;
}

// ============================================================================
// Result Types
// ============================================================================

/// Result of feasibility check
#[derive(Clone, Debug)]
pub struct FeasibilityResult {
    pub feasible: bool,
    pub conflicts: Vec<Conflict>,
    pub suggestions: Vec<Suggestion>,
}

/// Scheduling conflict
#[derive(Clone, Debug)]
pub struct Conflict {
    pub conflict_type: ConflictType,
    pub description: String,
    pub involved_tasks: Vec<TaskId>,
    pub involved_resources: Vec<ResourceId>,
}

#[derive(Clone, Debug)]
pub enum ConflictType {
    CircularDependency,
    ResourceOverallocation,
    ImpossibleConstraint,
    DeadlineMissed,
}

/// Suggestion for resolving issues
#[derive(Clone, Debug)]
pub struct Suggestion {
    pub description: String,
    pub impact: String,
}

/// Explanation of a scheduling decision
#[derive(Clone, Debug)]
pub struct Explanation {
    pub task_id: TaskId,
    pub reason: String,
    pub constraints_applied: Vec<String>,
    pub alternatives_considered: Vec<String>,
}

/// Constraint for what-if analysis
#[derive(Clone, Debug)]
pub enum Constraint {
    TaskEffort { task_id: TaskId, effort: Duration },
    TaskDuration { task_id: TaskId, duration: Duration },
    ResourceCapacity { resource_id: ResourceId, capacity: f32 },
    Deadline { date: NaiveDate },
}

/// Result of what-if analysis
#[derive(Clone, Debug)]
pub struct WhatIfReport {
    pub still_feasible: bool,
    pub solutions_before: num_bigint::BigUint,
    pub solutions_after: num_bigint::BigUint,
    pub newly_critical: Vec<TaskId>,
    pub schedule_delta: Option<Duration>,
    pub cost_delta: Option<Money>,
}

// ============================================================================
// Errors
// ============================================================================

/// Scheduling error
#[derive(Debug, Error)]
pub enum ScheduleError {
    #[error("Circular dependency detected: {0}")]
    CircularDependency(String),

    #[error("Resource not found: {0}")]
    ResourceNotFound(ResourceId),

    #[error("Task not found: {0}")]
    TaskNotFound(TaskId),

    #[error("Calendar not found: {0}")]
    CalendarNotFound(CalendarId),

    #[error("Infeasible schedule: {0}")]
    Infeasible(String),

    #[error("Constraint violation: {0}")]
    ConstraintViolation(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

/// Rendering error
#[derive(Debug, Error)]
pub enum RenderError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Format error: {0}")]
    Format(String),

    #[error("Invalid data: {0}")]
    InvalidData(String),
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duration_arithmetic() {
        let d1 = Duration::days(5);
        let d2 = Duration::days(3);
        assert_eq!((d1 + d2).as_days(), 8.0);
        assert_eq!((d1 - d2).as_days(), 2.0);
    }

    #[test]
    fn task_builder() {
        let task = Task::new("impl")
            .name("Implementation")
            .effort(Duration::days(10))
            .depends_on("design")
            .assign("dev")
            .priority(700);

        assert_eq!(task.id, "impl");
        assert_eq!(task.name, "Implementation");
        assert_eq!(task.effort, Some(Duration::days(10)));
        assert_eq!(task.depends.len(), 1);
        assert_eq!(task.assigned.len(), 1);
        assert_eq!(task.priority, 700);
    }

    #[test]
    fn calendar_working_day() {
        let cal = Calendar::default();
        
        // Monday
        let monday = NaiveDate::from_ymd_opt(2025, 2, 3).unwrap();
        assert!(cal.is_working_day(monday));
        
        // Saturday
        let saturday = NaiveDate::from_ymd_opt(2025, 2, 1).unwrap();
        assert!(!cal.is_working_day(saturday));
    }

    #[test]
    fn project_leaf_tasks() {
        let project = Project {
            id: "test".into(),
            name: "Test".into(),
            start: NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
            end: None,
            calendar: "default".into(),
            currency: "USD".into(),
            tasks: vec![
                Task::new("parent")
                    .child(Task::new("child1"))
                    .child(Task::new("child2")),
                Task::new("standalone"),
            ],
            resources: Vec::new(),
            calendars: vec![Calendar::default()],
            scenarios: Vec::new(),
            attributes: HashMap::new(),
        };

        let leaves = project.leaf_tasks();
        assert_eq!(leaves.len(), 3);
        assert!(leaves.iter().any(|t| t.id == "child1"));
        assert!(leaves.iter().any(|t| t.id == "child2"));
        assert!(leaves.iter().any(|t| t.id == "standalone"));
    }

    #[test]
    fn duration_constructors() {
        // Test minutes constructor
        let d_min = Duration::minutes(120);
        assert_eq!(d_min.minutes, 120);
        assert_eq!(d_min.as_hours(), 2.0);

        // Test hours constructor
        let d_hours = Duration::hours(3);
        assert_eq!(d_hours.minutes, 180);
        assert_eq!(d_hours.as_hours(), 3.0);

        // Test weeks constructor (5 days * 8 hours)
        let d_weeks = Duration::weeks(1);
        assert_eq!(d_weeks.minutes, 5 * 8 * 60);
        assert_eq!(d_weeks.as_days(), 5.0);
    }

    #[test]
    fn project_get_task_nested() {
        let project = Project {
            id: "test".into(),
            name: "Test".into(),
            start: NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
            end: None,
            calendar: "default".into(),
            currency: "USD".into(),
            tasks: vec![
                Task::new("parent")
                    .name("Parent Task")
                    .child(Task::new("child1").name("Child 1"))
                    .child(Task::new("child2")
                        .name("Child 2")
                        .child(Task::new("grandchild").name("Grandchild"))),
                Task::new("standalone").name("Standalone"),
            ],
            resources: Vec::new(),
            calendars: vec![Calendar::default()],
            scenarios: Vec::new(),
            attributes: HashMap::new(),
        };

        // Find top-level task
        let standalone = project.get_task("standalone");
        assert!(standalone.is_some());
        assert_eq!(standalone.unwrap().name, "Standalone");

        // Find nested task (depth 1)
        let child1 = project.get_task("child1");
        assert!(child1.is_some());
        assert_eq!(child1.unwrap().name, "Child 1");

        // Find deeply nested task (depth 2)
        let grandchild = project.get_task("grandchild");
        assert!(grandchild.is_some());
        assert_eq!(grandchild.unwrap().name, "Grandchild");

        // Non-existent task
        let missing = project.get_task("nonexistent");
        assert!(missing.is_none());
    }

    #[test]
    fn project_get_resource() {
        let project = Project {
            id: "test".into(),
            name: "Test".into(),
            start: NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
            end: None,
            calendar: "default".into(),
            currency: "USD".into(),
            tasks: Vec::new(),
            resources: vec![
                Resource::new("dev1").name("Developer 1"),
                Resource::new("pm").name("Project Manager"),
            ],
            calendars: vec![Calendar::default()],
            scenarios: Vec::new(),
            attributes: HashMap::new(),
        };

        let dev = project.get_resource("dev1");
        assert!(dev.is_some());
        assert_eq!(dev.unwrap().name, "Developer 1");

        let pm = project.get_resource("pm");
        assert!(pm.is_some());
        assert_eq!(pm.unwrap().name, "Project Manager");

        let missing = project.get_resource("nonexistent");
        assert!(missing.is_none());
    }

    #[test]
    fn task_is_summary() {
        let leaf_task = Task::new("leaf").name("Leaf Task");
        assert!(!leaf_task.is_summary());

        let summary_task = Task::new("summary")
            .name("Summary Task")
            .child(Task::new("child1"))
            .child(Task::new("child2"));
        assert!(summary_task.is_summary());
    }

    #[test]
    fn task_assign_with_units() {
        let task = Task::new("task1")
            .assign("dev1")
            .assign_with_units("dev2", 0.5)
            .assign_with_units("contractor", 0.25);

        assert_eq!(task.assigned.len(), 3);
        assert_eq!(task.assigned[0].units, 1.0); // Default assignment
        assert_eq!(task.assigned[1].units, 0.5); // Partial assignment
        assert_eq!(task.assigned[2].units, 0.25); // Quarter assignment
    }

    #[test]
    fn resource_efficiency() {
        let resource = Resource::new("dev")
            .name("Developer")
            .efficiency(0.8);

        assert_eq!(resource.efficiency, 0.8);
    }

    #[test]
    fn calendar_hours_per_day() {
        let cal = Calendar::default();
        // Default: 9:00-12:00 (3h) + 13:00-17:00 (4h) = 7 hours
        assert_eq!(cal.hours_per_day(), 7.0);
    }

    #[test]
    fn time_range_duration() {
        let range = TimeRange {
            start: 9 * 60,  // 9:00 AM
            end: 17 * 60,   // 5:00 PM
        };
        assert_eq!(range.duration_hours(), 8.0);

        let half_day = TimeRange {
            start: 9 * 60,
            end: 13 * 60,
        };
        assert_eq!(half_day.duration_hours(), 4.0);
    }

    #[test]
    fn holiday_contains_date() {
        let holiday = Holiday {
            name: "Winter Break".into(),
            start: NaiveDate::from_ymd_opt(2025, 12, 24).unwrap(),
            end: NaiveDate::from_ymd_opt(2025, 12, 26).unwrap(),
        };

        // Before holiday
        assert!(!holiday.contains(NaiveDate::from_ymd_opt(2025, 12, 23).unwrap()));

        // First day of holiday
        assert!(holiday.contains(NaiveDate::from_ymd_opt(2025, 12, 24).unwrap()));

        // Middle of holiday
        assert!(holiday.contains(NaiveDate::from_ymd_opt(2025, 12, 25).unwrap()));

        // Last day of holiday
        assert!(holiday.contains(NaiveDate::from_ymd_opt(2025, 12, 26).unwrap()));

        // After holiday
        assert!(!holiday.contains(NaiveDate::from_ymd_opt(2025, 12, 27).unwrap()));
    }

    #[test]
    fn task_milestone() {
        let milestone = Task::new("ms1")
            .name("Phase Complete")
            .milestone();

        assert!(milestone.milestone);
        assert_eq!(milestone.duration, Some(Duration::zero()));
    }

    #[test]
    fn depends_on_creates_fs_dependency() {
        let task = Task::new("task").depends_on("pred");

        assert_eq!(task.depends.len(), 1);
        let dep = &task.depends[0];
        assert_eq!(dep.predecessor, "pred");
        assert_eq!(dep.dep_type, DependencyType::FinishToStart);
        assert!(dep.lag.is_none());
    }

    #[test]
    fn with_dependency_preserves_all_fields() {
        let dep = Dependency {
            predecessor: "other".into(),
            dep_type: DependencyType::StartToStart,
            lag: Some(Duration::days(2)),
        };
        let task = Task::new("task").with_dependency(dep);

        assert_eq!(task.depends.len(), 1);
        let d = &task.depends[0];
        assert_eq!(d.predecessor, "other");
        assert_eq!(d.dep_type, DependencyType::StartToStart);
        assert_eq!(d.lag, Some(Duration::days(2)));
    }

    #[test]
    fn assign_sets_full_allocation() {
        let task = Task::new("task").assign("dev");

        assert_eq!(task.assigned.len(), 1);
        assert_eq!(task.assigned[0].resource_id, "dev");
        assert_eq!(task.assigned[0].units, 1.0);
    }

    #[test]
    fn assign_with_units_sets_custom_allocation() {
        let task = Task::new("task").assign_with_units("dev", 0.75);

        assert_eq!(task.assigned.len(), 1);
        assert_eq!(task.assigned[0].resource_id, "dev");
        assert_eq!(task.assigned[0].units, 0.75);
    }

    // ========================================================================
    // Progress Tracking Tests
    // ========================================================================

    #[test]
    fn remaining_duration_linear_interpolation() {
        // 10-day task at 60% complete → 4 days remaining
        let task = Task::new("task")
            .duration(Duration::days(10))
            .complete(60.0);

        let remaining = task.remaining_duration();
        assert_eq!(remaining.as_days(), 4.0);
    }

    #[test]
    fn remaining_duration_zero_complete() {
        let task = Task::new("task")
            .duration(Duration::days(10));

        let remaining = task.remaining_duration();
        assert_eq!(remaining.as_days(), 10.0);
    }

    #[test]
    fn remaining_duration_fully_complete() {
        let task = Task::new("task")
            .duration(Duration::days(10))
            .complete(100.0);

        let remaining = task.remaining_duration();
        assert_eq!(remaining.as_days(), 0.0);
    }

    #[test]
    fn remaining_duration_uses_effort_if_no_duration() {
        let task = Task::new("task")
            .effort(Duration::days(20))
            .complete(50.0);

        let remaining = task.remaining_duration();
        assert_eq!(remaining.as_days(), 10.0);
    }

    #[test]
    fn effective_percent_complete_default() {
        let task = Task::new("task");
        assert_eq!(task.effective_percent_complete(), 0);
    }

    #[test]
    fn effective_percent_complete_clamped() {
        // Clamp above 100
        let task = Task::new("task").complete(150.0);
        assert_eq!(task.effective_percent_complete(), 100);

        // Clamp below 0
        let task = Task::new("task").complete(-10.0);
        assert_eq!(task.effective_percent_complete(), 0);
    }

    #[test]
    fn derived_status_not_started() {
        let task = Task::new("task");
        assert_eq!(task.derived_status(), TaskStatus::NotStarted);
    }

    #[test]
    fn derived_status_in_progress_from_percent() {
        let task = Task::new("task").complete(50.0);
        assert_eq!(task.derived_status(), TaskStatus::InProgress);
    }

    #[test]
    fn derived_status_in_progress_from_actual_start() {
        let task = Task::new("task")
            .actual_start(NaiveDate::from_ymd_opt(2026, 1, 15).unwrap());
        assert_eq!(task.derived_status(), TaskStatus::InProgress);
    }

    #[test]
    fn derived_status_complete_from_percent() {
        let task = Task::new("task").complete(100.0);
        assert_eq!(task.derived_status(), TaskStatus::Complete);
    }

    #[test]
    fn derived_status_complete_from_actual_finish() {
        let task = Task::new("task")
            .actual_finish(NaiveDate::from_ymd_opt(2026, 1, 20).unwrap());
        assert_eq!(task.derived_status(), TaskStatus::Complete);
    }

    #[test]
    fn derived_status_explicit_overrides() {
        // Even with 100% complete, explicit status takes precedence
        let task = Task::new("task")
            .complete(100.0)
            .with_status(TaskStatus::Blocked);
        assert_eq!(task.derived_status(), TaskStatus::Blocked);
    }

    #[test]
    fn task_status_display() {
        assert_eq!(format!("{}", TaskStatus::NotStarted), "Not Started");
        assert_eq!(format!("{}", TaskStatus::InProgress), "In Progress");
        assert_eq!(format!("{}", TaskStatus::Complete), "Complete");
        assert_eq!(format!("{}", TaskStatus::Blocked), "Blocked");
        assert_eq!(format!("{}", TaskStatus::AtRisk), "At Risk");
        assert_eq!(format!("{}", TaskStatus::OnHold), "On Hold");
    }

    #[test]
    fn task_builder_with_progress_fields() {
        let date_start = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
        let date_finish = NaiveDate::from_ymd_opt(2026, 1, 20).unwrap();

        let task = Task::new("task")
            .duration(Duration::days(5))
            .complete(75.0)
            .actual_start(date_start)
            .actual_finish(date_finish)
            .with_status(TaskStatus::Complete);

        assert_eq!(task.complete, Some(75.0));
        assert_eq!(task.actual_start, Some(date_start));
        assert_eq!(task.actual_finish, Some(date_finish));
        assert_eq!(task.status, Some(TaskStatus::Complete));
    }

    // ========================================================================
    // Container Progress Tests
    // ========================================================================

    #[test]
    fn container_progress_weighted_average() {
        // Container with 3 children of different durations and progress
        // Backend: 20d @ 60%, Frontend: 15d @ 30%, Testing: 10d @ 0%
        // Expected: (20*60 + 15*30 + 10*0) / (20+15+10) = 1650/45 = 36.67 ≈ 37%
        let container = Task::new("development")
            .child(Task::new("backend").duration(Duration::days(20)).complete(60.0))
            .child(Task::new("frontend").duration(Duration::days(15)).complete(30.0))
            .child(Task::new("testing").duration(Duration::days(10)));

        assert!(container.is_container());
        assert_eq!(container.container_progress(), Some(37));
    }

    #[test]
    fn container_progress_empty_container() {
        let container = Task::new("empty");
        assert!(!container.is_container());
        assert_eq!(container.container_progress(), None);
    }

    #[test]
    fn container_progress_all_complete() {
        let container = Task::new("done")
            .child(Task::new("a").duration(Duration::days(5)).complete(100.0))
            .child(Task::new("b").duration(Duration::days(5)).complete(100.0));

        assert_eq!(container.container_progress(), Some(100));
    }

    #[test]
    fn container_progress_none_started() {
        let container = Task::new("pending")
            .child(Task::new("a").duration(Duration::days(5)))
            .child(Task::new("b").duration(Duration::days(5)));

        assert_eq!(container.container_progress(), Some(0));
    }

    #[test]
    fn container_progress_nested_containers() {
        // Nested structure:
        // project
        // ├── phase1 (container)
        // │   ├── task_a: 10d @ 100%
        // │   └── task_b: 10d @ 50%
        // └── phase2 (container)
        //     └── task_c: 20d @ 25%
        //
        // Phase1: (10*100 + 10*50) / 20 = 75%
        // Total: (10*100 + 10*50 + 20*25) / 40 = 2000/40 = 50%
        let project = Task::new("project")
            .child(
                Task::new("phase1")
                    .child(Task::new("task_a").duration(Duration::days(10)).complete(100.0))
                    .child(Task::new("task_b").duration(Duration::days(10)).complete(50.0)),
            )
            .child(
                Task::new("phase2")
                    .child(Task::new("task_c").duration(Duration::days(20)).complete(25.0)),
            );

        // Check nested container progress
        let phase1 = &project.children[0];
        assert_eq!(phase1.container_progress(), Some(75));

        // Check top-level container progress (flattens all leaves)
        assert_eq!(project.container_progress(), Some(50));
    }

    #[test]
    fn container_progress_effective_with_override() {
        // Container with explicit progress set (manual override)
        let container = Task::new("dev")
            .complete(80.0) // Manual override
            .child(Task::new("a").duration(Duration::days(10)).complete(50.0))
            .child(Task::new("b").duration(Duration::days(10)).complete(50.0));

        // Derived would be 50%, but manual override is 80%
        assert_eq!(container.container_progress(), Some(50));
        assert_eq!(container.effective_progress(), 80); // Uses override
    }

    #[test]
    fn container_progress_mismatch_detection() {
        let container = Task::new("dev")
            .complete(80.0) // Claims 80%
            .child(Task::new("a").duration(Duration::days(10)).complete(30.0))
            .child(Task::new("b").duration(Duration::days(10)).complete(30.0));

        // Derived is 30%, claimed is 80% - 50% mismatch
        let mismatch = container.progress_mismatch(20);
        assert!(mismatch.is_some());
        let (manual, derived) = mismatch.unwrap();
        assert_eq!(manual, 80);
        assert_eq!(derived, 30);

        // No mismatch if threshold is high
        assert!(container.progress_mismatch(60).is_none());
    }

    #[test]
    fn container_progress_uses_effort_fallback() {
        // When duration not set, should use effort
        let container = Task::new("dev")
            .child(Task::new("a").effort(Duration::days(5)).complete(100.0))
            .child(Task::new("b").effort(Duration::days(5)).complete(0.0));

        assert_eq!(container.container_progress(), Some(50));
    }

    #[test]
    fn container_progress_zero_duration_children() {
        // Container with children that have no duration/effort returns None
        let container = Task::new("dev")
            .child(Task::new("a").complete(50.0))  // No duration
            .child(Task::new("b").complete(100.0)); // No duration

        assert_eq!(container.container_progress(), None);
    }

    #[test]
    fn effective_progress_container_no_override() {
        // Container without manual override uses derived progress
        let container = Task::new("dev")
            .child(Task::new("a").duration(Duration::days(10)).complete(100.0))
            .child(Task::new("b").duration(Duration::days(10)).complete(0.0));

        // No complete() set on container, so it derives from children
        assert_eq!(container.effective_progress(), 50);
    }

    #[test]
    fn effective_progress_leaf_no_complete() {
        // Leaf task with no complete set returns 0
        let task = Task::new("leaf").duration(Duration::days(5));
        assert_eq!(task.effective_progress(), 0);
    }

    #[test]
    fn progress_mismatch_leaf_returns_none() {
        // progress_mismatch on a leaf task returns None
        let task = Task::new("leaf").duration(Duration::days(5)).complete(50.0);
        assert!(task.progress_mismatch(10).is_none());
    }

    #[test]
    fn money_new() {
        use rust_decimal::Decimal;
        use std::str::FromStr;
        let money = Money::new(Decimal::from_str("100.50").unwrap(), "EUR");
        assert_eq!(money.amount, Decimal::from_str("100.50").unwrap());
        assert_eq!(money.currency, "EUR");
    }

    #[test]
    fn resource_rate() {
        use rust_decimal::Decimal;
        use std::str::FromStr;
        let resource = Resource::new("dev")
            .name("Developer")
            .rate(Money::new(Decimal::from_str("500").unwrap(), "USD"));

        assert!(resource.rate.is_some());
        assert_eq!(resource.rate.unwrap().amount, Decimal::from_str("500").unwrap());
    }

    #[test]
    fn calendar_with_holiday() {
        let mut cal = Calendar::default();
        cal.holidays.push(Holiday {
            name: "New Year".into(),
            start: NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
            end: NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
        });

        // Jan 1 is a Wednesday (working day) but is a holiday
        let jan1 = NaiveDate::from_ymd_opt(2025, 1, 1).unwrap();
        assert!(!cal.is_working_day(jan1));

        // Jan 2 is Thursday, should be working
        let jan2 = NaiveDate::from_ymd_opt(2025, 1, 2).unwrap();
        assert!(cal.is_working_day(jan2));
    }

    #[test]
    fn scheduled_task_test_new() {
        let start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        let finish = NaiveDate::from_ymd_opt(2025, 1, 10).unwrap();
        let st = ScheduledTask::test_new("task1", start, finish, Duration::days(5), Duration::zero(), true);

        assert_eq!(st.task_id, "task1");
        assert_eq!(st.start, start);
        assert_eq!(st.finish, finish);
        assert!(st.is_critical);
        assert_eq!(st.assignments.len(), 0);
        assert_eq!(st.percent_complete, 0);
        assert_eq!(st.status, TaskStatus::NotStarted);
    }
}
