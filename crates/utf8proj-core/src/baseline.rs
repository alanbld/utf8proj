//! Baseline Management for Schedule Snapshots (RFC-0013)
//!
//! This module provides types for capturing and comparing schedule snapshots
//! (baselines). Baselines answer the fundamental project question: "Compared to what?"
//!
//! # Core Concepts
//!
//! - **Baseline**: A frozen snapshot of computed schedule dates
//! - **TaskSnapshot**: Per-task capture of early start/finish dates
//! - **BaselineStore**: Collection of baselines for a project
//! - **ScheduleComparison**: Result of comparing current schedule to baseline
//!
//! # Design Principles
//!
//! - **Immutability**: Baselines cannot be updated, only deleted and recreated
//! - **Leaf-only**: Only leaf tasks are baselined; containers are derived
//! - **Stable IDs**: Tasks identified by fully-qualified ID path
//! - **Output-based**: Captures computed dates, not planning inputs
//!
//! # Example
//!
//! ```rust
//! use chrono::{NaiveDate, Utc};
//! use utf8proj_core::baseline::{Baseline, TaskSnapshot, BaselineStore};
//! use std::collections::BTreeMap;
//!
//! // Create a baseline
//! let mut tasks = BTreeMap::new();
//! tasks.insert("design".to_string(), TaskSnapshot {
//!     task_id: "design".to_string(),
//!     start: NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
//!     finish: NaiveDate::from_ymd_opt(2026, 1, 10).unwrap(),
//! });
//!
//! let baseline = Baseline {
//!     name: "original".to_string(),
//!     saved: Utc::now(),
//!     description: Some("Initial approved plan".to_string()),
//!     parent: None,
//!     tasks,
//!     project_finish: NaiveDate::from_ymd_opt(2026, 3, 1).unwrap(),
//! };
//!
//! // Store it
//! let mut store = BaselineStore::new();
//! store.add(baseline).unwrap();
//!
//! assert!(store.get("original").is_some());
//! ```

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

// ============================================================================
// Core Types
// ============================================================================

/// A named schedule snapshot (RFC-0013)
///
/// Baselines capture the computed scheduling result (output), not the planning
/// inputs. They store per-task early start/finish dates as computed by CPM.
///
/// Baselines are immutable: they cannot be updated, only deleted and recreated.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Baseline {
    /// Unique baseline identifier
    pub name: String,

    /// UTC timestamp when the baseline was saved
    pub saved: DateTime<Utc>,

    /// Human-readable description
    pub description: Option<String>,

    /// Parent baseline name (conceptual lineage only, not inheritance)
    pub parent: Option<String>,

    /// Leaf task snapshots, sorted by task ID
    pub tasks: BTreeMap<String, TaskSnapshot>,

    /// Project finish date (max of all task finish dates)
    pub project_finish: NaiveDate,
}

impl Baseline {
    /// Create a new baseline with the given name and current timestamp
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            saved: Utc::now(),
            description: None,
            parent: None,
            tasks: BTreeMap::new(),
            project_finish: NaiveDate::from_ymd_opt(1970, 1, 1).unwrap(),
        }
    }

    /// Set the description
    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    /// Set the parent baseline
    pub fn parent(mut self, parent: impl Into<String>) -> Self {
        self.parent = Some(parent.into());
        self
    }

    /// Add a task snapshot
    pub fn add_task(&mut self, snapshot: TaskSnapshot) {
        // Update project finish if this task finishes later
        if snapshot.finish > self.project_finish {
            self.project_finish = snapshot.finish;
        }
        self.tasks.insert(snapshot.task_id.clone(), snapshot);
    }

    /// Get the number of tasks in this baseline
    pub fn task_count(&self) -> usize {
        self.tasks.len()
    }
}

/// Snapshot of a leaf task's scheduled dates (RFC-0013)
///
/// Captures the early start and early finish as computed by CPM scheduling.
/// Only leaf tasks are captured; container dates are derived from children.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskSnapshot {
    /// Fully-qualified task identifier (e.g., "phase1.design")
    pub task_id: String,

    /// Scheduled early start date
    pub start: NaiveDate,

    /// Scheduled early finish date
    pub finish: NaiveDate,
}

impl TaskSnapshot {
    /// Create a new task snapshot
    pub fn new(task_id: impl Into<String>, start: NaiveDate, finish: NaiveDate) -> Self {
        Self {
            task_id: task_id.into(),
            start,
            finish,
        }
    }
}

/// Collection of baselines for a project (RFC-0013)
///
/// Stored in a `.baselines` sidecar file alongside the main `.proj` file.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct BaselineStore {
    /// Baselines indexed by name, sorted alphabetically
    pub baselines: BTreeMap<String, Baseline>,
}

impl BaselineStore {
    /// Create an empty baseline store
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a baseline to the store
    ///
    /// Returns an error if a baseline with the same name already exists.
    pub fn add(&mut self, baseline: Baseline) -> Result<(), BaselineError> {
        if self.baselines.contains_key(&baseline.name) {
            return Err(BaselineError::AlreadyExists(baseline.name));
        }
        self.baselines.insert(baseline.name.clone(), baseline);
        Ok(())
    }

    /// Get a baseline by name
    pub fn get(&self, name: &str) -> Option<&Baseline> {
        self.baselines.get(name)
    }

    /// Remove a baseline by name
    ///
    /// Returns the removed baseline, or None if not found.
    pub fn remove(&mut self, name: &str) -> Option<Baseline> {
        self.baselines.remove(name)
    }

    /// Check if a baseline with the given name exists
    pub fn contains(&self, name: &str) -> bool {
        self.baselines.contains_key(name)
    }

    /// Get the number of baselines in the store
    pub fn len(&self) -> usize {
        self.baselines.len()
    }

    /// Check if the store is empty
    pub fn is_empty(&self) -> bool {
        self.baselines.is_empty()
    }

    /// Iterate over all baselines in alphabetical order by name
    pub fn iter(&self) -> impl Iterator<Item = (&String, &Baseline)> {
        self.baselines.iter()
    }

    /// List all baseline names
    pub fn names(&self) -> Vec<&str> {
        self.baselines.keys().map(String::as_str).collect()
    }
}

// ============================================================================
// Comparison Types
// ============================================================================

/// Result of comparing current schedule against a baseline (RFC-0013)
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ScheduleComparison {
    /// Name of the baseline compared against
    pub baseline_name: String,

    /// When the baseline was saved
    pub baseline_saved: DateTime<Utc>,

    /// When the comparison was performed
    pub comparison_date: DateTime<Utc>,

    /// Per-task variance information
    pub tasks: Vec<TaskVariance>,

    /// Aggregated comparison summary
    pub summary: ComparisonSummary,
}

/// Variance information for a single task (RFC-0013)
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskVariance {
    /// Task identifier
    pub task_id: String,

    /// Baseline start date (None if task was added after baseline)
    pub baseline_start: Option<NaiveDate>,

    /// Baseline finish date (None if task was added after baseline)
    pub baseline_finish: Option<NaiveDate>,

    /// Current scheduled start date (None if task was removed)
    pub current_start: Option<NaiveDate>,

    /// Current scheduled finish date (None if task was removed)
    pub current_finish: Option<NaiveDate>,

    /// Start variance in calendar days (current - baseline)
    pub start_variance_days: Option<i32>,

    /// Finish variance in calendar days (current - baseline)
    pub finish_variance_days: Option<i32>,

    /// Variance status classification
    pub status: VarianceStatus,
}

impl TaskVariance {
    /// Create a variance for a task that exists in both baseline and current
    pub fn existing(
        task_id: impl Into<String>,
        baseline_start: NaiveDate,
        baseline_finish: NaiveDate,
        current_start: NaiveDate,
        current_finish: NaiveDate,
    ) -> Self {
        let start_variance_days = (current_start - baseline_start).num_days() as i32;
        let finish_variance_days = (current_finish - baseline_finish).num_days() as i32;

        let status = match finish_variance_days.cmp(&0) {
            std::cmp::Ordering::Greater => VarianceStatus::Delayed,
            std::cmp::Ordering::Less => VarianceStatus::Ahead,
            std::cmp::Ordering::Equal => VarianceStatus::OnSchedule,
        };

        Self {
            task_id: task_id.into(),
            baseline_start: Some(baseline_start),
            baseline_finish: Some(baseline_finish),
            current_start: Some(current_start),
            current_finish: Some(current_finish),
            start_variance_days: Some(start_variance_days),
            finish_variance_days: Some(finish_variance_days),
            status,
        }
    }

    /// Create a variance for a task that was added after the baseline
    pub fn added(
        task_id: impl Into<String>,
        current_start: NaiveDate,
        current_finish: NaiveDate,
    ) -> Self {
        Self {
            task_id: task_id.into(),
            baseline_start: None,
            baseline_finish: None,
            current_start: Some(current_start),
            current_finish: Some(current_finish),
            start_variance_days: None,
            finish_variance_days: None,
            status: VarianceStatus::Added,
        }
    }

    /// Create a variance for a task that was removed (exists in baseline but not current)
    pub fn removed(
        task_id: impl Into<String>,
        baseline_start: NaiveDate,
        baseline_finish: NaiveDate,
    ) -> Self {
        Self {
            task_id: task_id.into(),
            baseline_start: Some(baseline_start),
            baseline_finish: Some(baseline_finish),
            current_start: None,
            current_finish: None,
            start_variance_days: None,
            finish_variance_days: None,
            status: VarianceStatus::Removed,
        }
    }
}

/// Classification of task variance status (RFC-0013)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum VarianceStatus {
    /// Task finish date matches baseline (variance = 0)
    OnSchedule,
    /// Task finish date is later than baseline (variance > 0)
    Delayed,
    /// Task finish date is earlier than baseline (variance < 0)
    Ahead,
    /// Task exists in current schedule but not in baseline
    Added,
    /// Task exists in baseline but not in current schedule
    Removed,
}

impl VarianceStatus {
    /// Get a display string for the status
    pub fn as_str(&self) -> &'static str {
        match self {
            VarianceStatus::OnSchedule => "on_schedule",
            VarianceStatus::Delayed => "delayed",
            VarianceStatus::Ahead => "ahead",
            VarianceStatus::Added => "added",
            VarianceStatus::Removed => "removed",
        }
    }
}

impl std::fmt::Display for VarianceStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Aggregated comparison summary (RFC-0013)
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComparisonSummary {
    /// Number of tasks compared (exist in both baseline and current)
    pub tasks_compared: usize,

    /// Number of tasks on schedule
    pub tasks_on_schedule: usize,

    /// Number of delayed tasks
    pub tasks_delayed: usize,

    /// Number of tasks ahead of schedule
    pub tasks_ahead: usize,

    /// Number of tasks added since baseline
    pub tasks_added: usize,

    /// Number of tasks removed since baseline
    pub tasks_removed: usize,

    /// Baseline project finish date
    pub baseline_project_finish: NaiveDate,

    /// Current project finish date
    pub current_project_finish: NaiveDate,

    /// Project-level variance in calendar days
    pub project_variance_days: i32,
}

impl Default for ComparisonSummary {
    fn default() -> Self {
        let epoch = NaiveDate::from_ymd_opt(1970, 1, 1).unwrap();
        Self {
            tasks_compared: 0,
            tasks_on_schedule: 0,
            tasks_delayed: 0,
            tasks_ahead: 0,
            tasks_added: 0,
            tasks_removed: 0,
            baseline_project_finish: epoch,
            current_project_finish: epoch,
            project_variance_days: 0,
        }
    }
}

// ============================================================================
// Error Types
// ============================================================================

/// Errors that can occur during baseline operations
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BaselineError {
    /// Baseline with this name already exists
    AlreadyExists(String),
    /// Baseline not found
    NotFound(String),
    /// No baselines file exists for the project
    NoBaselinesFile,
    /// Tasks cannot be baselined due to missing IDs
    TasksWithoutIds(usize),
}

impl std::fmt::Display for BaselineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BaselineError::AlreadyExists(name) => {
                write!(f, "Baseline \"{}\" already exists", name)
            }
            BaselineError::NotFound(name) => {
                write!(f, "Baseline \"{}\" not found", name)
            }
            BaselineError::NoBaselinesFile => {
                write!(f, "No baselines file found for this project")
            }
            BaselineError::TasksWithoutIds(count) => {
                write!(f, "Cannot baseline: {} tasks have no ID", count)
            }
        }
    }
}

impl std::error::Error for BaselineError {}

// ============================================================================
// Comparison Functions
// ============================================================================

use crate::{Project, Schedule, Task};

/// Compare a current schedule against a baseline (RFC-0013)
///
/// This function computes variance between the current scheduled dates and
/// the baseline dates. Tasks are matched by ID.
///
/// # Arguments
///
/// * `schedule` - The current schedule
/// * `baseline` - The baseline to compare against
/// * `project` - The original project (used to identify container tasks)
///
/// # Returns
///
/// A `ScheduleComparison` containing per-task variances and summary statistics.
///
/// # Example
///
/// ```ignore
/// let comparison = compare_schedule_to_baseline(&schedule, &baseline, &project);
/// println!("Project slip: {} days", comparison.summary.project_variance_days);
/// ```
pub fn compare_schedule_to_baseline(
    schedule: &Schedule,
    baseline: &Baseline,
    project: &Project,
) -> ScheduleComparison {
    let container_ids = collect_container_ids(project);
    let mut variances = Vec::new();

    // Check tasks in current schedule against baseline
    for (task_id, scheduled_task) in &schedule.tasks {
        // Skip containers - they're not in baselines
        if container_ids.contains(task_id) {
            continue;
        }

        match baseline.tasks.get(task_id) {
            Some(baseline_snap) => {
                // Task exists in both - compute variance
                variances.push(TaskVariance::existing(
                    task_id,
                    baseline_snap.start,
                    baseline_snap.finish,
                    scheduled_task.early_start,
                    scheduled_task.early_finish,
                ));
            }
            None => {
                // Task added since baseline
                variances.push(TaskVariance::added(
                    task_id,
                    scheduled_task.early_start,
                    scheduled_task.early_finish,
                ));
            }
        }
    }

    // Check for removed tasks (in baseline but not in current)
    for (task_id, baseline_snap) in &baseline.tasks {
        if !schedule.tasks.contains_key(task_id) {
            variances.push(TaskVariance::removed(
                task_id,
                baseline_snap.start,
                baseline_snap.finish,
            ));
        }
    }

    // Sort variances by task ID for deterministic output
    variances.sort_by(|a, b| a.task_id.cmp(&b.task_id));

    // Compute summary
    let summary = compute_summary(&variances, baseline, schedule);

    ScheduleComparison {
        baseline_name: baseline.name.clone(),
        baseline_saved: baseline.saved,
        comparison_date: Utc::now(),
        tasks: variances,
        summary,
    }
}

/// Collect task IDs of all container (non-leaf) tasks
fn collect_container_ids(project: &Project) -> std::collections::HashSet<String> {
    let mut containers = std::collections::HashSet::new();
    for task in &project.tasks {
        collect_containers_recursive(task, "", &mut containers);
    }
    containers
}

fn collect_containers_recursive(
    task: &Task,
    prefix: &str,
    containers: &mut std::collections::HashSet<String>,
) {
    let qualified_id = if prefix.is_empty() {
        task.id.clone()
    } else {
        format!("{}.{}", prefix, task.id)
    };

    if task.is_container() {
        containers.insert(qualified_id.clone());
    }

    for child in &task.children {
        collect_containers_recursive(child, &qualified_id, containers);
    }
}

/// Extract leaf tasks from a schedule for baseline creation
///
/// Returns a mapping of fully-qualified task IDs to `TaskSnapshot`s.
/// Container tasks are excluded.
pub fn extract_leaf_tasks(
    schedule: &Schedule,
    project: &Project,
) -> BTreeMap<String, TaskSnapshot> {
    let container_ids = collect_container_ids(project);
    let mut tasks = BTreeMap::new();

    for (task_id, scheduled_task) in &schedule.tasks {
        if !container_ids.contains(task_id) {
            tasks.insert(
                task_id.clone(),
                TaskSnapshot::new(
                    task_id,
                    scheduled_task.early_start,
                    scheduled_task.early_finish,
                ),
            );
        }
    }

    tasks
}

/// Count how many container tasks would be excluded from a baseline
pub fn count_containers(project: &Project) -> usize {
    collect_container_ids(project).len()
}

fn compute_summary(
    variances: &[TaskVariance],
    baseline: &Baseline,
    schedule: &Schedule,
) -> ComparisonSummary {
    let mut summary = ComparisonSummary {
        baseline_project_finish: baseline.project_finish,
        current_project_finish: schedule.project_end,
        ..Default::default()
    };

    for variance in variances {
        match variance.status {
            VarianceStatus::OnSchedule => {
                summary.tasks_compared += 1;
                summary.tasks_on_schedule += 1;
            }
            VarianceStatus::Delayed => {
                summary.tasks_compared += 1;
                summary.tasks_delayed += 1;
            }
            VarianceStatus::Ahead => {
                summary.tasks_compared += 1;
                summary.tasks_ahead += 1;
            }
            VarianceStatus::Added => {
                summary.tasks_added += 1;
            }
            VarianceStatus::Removed => {
                summary.tasks_removed += 1;
            }
        }
    }

    summary.project_variance_days =
        (schedule.project_end - baseline.project_finish).num_days() as i32;

    summary
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn date(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).unwrap()
    }

    #[test]
    fn test_task_snapshot_new() {
        let snapshot = TaskSnapshot::new("design", date(2026, 1, 1), date(2026, 1, 10));
        assert_eq!(snapshot.task_id, "design");
        assert_eq!(snapshot.start, date(2026, 1, 1));
        assert_eq!(snapshot.finish, date(2026, 1, 10));
    }

    #[test]
    fn test_baseline_new() {
        let baseline = Baseline::new("original")
            .description("Initial plan")
            .parent("none");

        assert_eq!(baseline.name, "original");
        assert_eq!(baseline.description, Some("Initial plan".to_string()));
        assert_eq!(baseline.parent, Some("none".to_string()));
        assert!(baseline.tasks.is_empty());
    }

    #[test]
    fn test_baseline_add_task() {
        let mut baseline = Baseline::new("original");

        baseline.add_task(TaskSnapshot::new(
            "design",
            date(2026, 1, 1),
            date(2026, 1, 10),
        ));
        baseline.add_task(TaskSnapshot::new(
            "build",
            date(2026, 1, 11),
            date(2026, 2, 15),
        ));

        assert_eq!(baseline.task_count(), 2);
        assert_eq!(baseline.project_finish, date(2026, 2, 15));
    }

    #[test]
    fn test_baseline_store_add_and_get() {
        let mut store = BaselineStore::new();

        let baseline = Baseline::new("original");
        store.add(baseline.clone()).unwrap();

        assert!(store.contains("original"));
        assert_eq!(store.get("original").unwrap().name, "original");
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn test_baseline_store_duplicate_error() {
        let mut store = BaselineStore::new();

        store.add(Baseline::new("original")).unwrap();
        let result = store.add(Baseline::new("original"));

        assert!(matches!(result, Err(BaselineError::AlreadyExists(_))));
    }

    #[test]
    fn test_baseline_store_remove() {
        let mut store = BaselineStore::new();
        store.add(Baseline::new("original")).unwrap();

        let removed = store.remove("original");
        assert!(removed.is_some());
        assert!(!store.contains("original"));
    }

    #[test]
    fn test_baseline_store_names() {
        let mut store = BaselineStore::new();
        store.add(Baseline::new("original")).unwrap();
        store.add(Baseline::new("change_order")).unwrap();

        let names = store.names();
        assert_eq!(names, vec!["change_order", "original"]); // BTreeMap sorts
    }

    #[test]
    fn test_task_variance_existing_on_schedule() {
        let variance = TaskVariance::existing(
            "design",
            date(2026, 1, 1),
            date(2026, 1, 10),
            date(2026, 1, 1),
            date(2026, 1, 10),
        );

        assert_eq!(variance.status, VarianceStatus::OnSchedule);
        assert_eq!(variance.finish_variance_days, Some(0));
    }

    #[test]
    fn test_task_variance_existing_delayed() {
        let variance = TaskVariance::existing(
            "design",
            date(2026, 1, 1),
            date(2026, 1, 10),
            date(2026, 1, 3),
            date(2026, 1, 15),
        );

        assert_eq!(variance.status, VarianceStatus::Delayed);
        assert_eq!(variance.finish_variance_days, Some(5));
        assert_eq!(variance.start_variance_days, Some(2));
    }

    #[test]
    fn test_task_variance_existing_ahead() {
        let variance = TaskVariance::existing(
            "design",
            date(2026, 1, 5),
            date(2026, 1, 15),
            date(2026, 1, 1),
            date(2026, 1, 10),
        );

        assert_eq!(variance.status, VarianceStatus::Ahead);
        assert_eq!(variance.finish_variance_days, Some(-5));
    }

    #[test]
    fn test_task_variance_added() {
        let variance = TaskVariance::added("new_task", date(2026, 2, 1), date(2026, 2, 10));

        assert_eq!(variance.status, VarianceStatus::Added);
        assert!(variance.baseline_start.is_none());
        assert!(variance.baseline_finish.is_none());
        assert_eq!(variance.current_start, Some(date(2026, 2, 1)));
    }

    #[test]
    fn test_task_variance_removed() {
        let variance = TaskVariance::removed("old_task", date(2026, 1, 1), date(2026, 1, 10));

        assert_eq!(variance.status, VarianceStatus::Removed);
        assert!(variance.current_start.is_none());
        assert!(variance.current_finish.is_none());
        assert_eq!(variance.baseline_start, Some(date(2026, 1, 1)));
    }

    #[test]
    fn test_variance_status_display() {
        assert_eq!(VarianceStatus::OnSchedule.as_str(), "on_schedule");
        assert_eq!(VarianceStatus::Delayed.as_str(), "delayed");
        assert_eq!(VarianceStatus::Ahead.as_str(), "ahead");
        assert_eq!(VarianceStatus::Added.as_str(), "added");
        assert_eq!(VarianceStatus::Removed.as_str(), "removed");
    }

    #[test]
    fn test_baseline_error_display() {
        let err = BaselineError::AlreadyExists("original".to_string());
        assert_eq!(format!("{}", err), "Baseline \"original\" already exists");

        let err = BaselineError::NotFound("missing".to_string());
        assert_eq!(format!("{}", err), "Baseline \"missing\" not found");

        let err = BaselineError::NoBaselinesFile;
        assert_eq!(
            format!("{}", err),
            "No baselines file found for this project"
        );

        let err = BaselineError::TasksWithoutIds(5);
        assert_eq!(format!("{}", err), "Cannot baseline: 5 tasks have no ID");
    }

    #[test]
    fn test_comparison_summary_default() {
        let summary = ComparisonSummary::default();
        assert_eq!(summary.tasks_compared, 0);
        assert_eq!(summary.project_variance_days, 0);
    }
}
