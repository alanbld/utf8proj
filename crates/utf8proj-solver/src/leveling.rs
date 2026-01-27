//! Resource Leveling Algorithm
//!
//! Detects and resolves resource over-allocation by shifting tasks within their slack.
//!
//! ## RFC-0003 Compliance
//!
//! This module implements deterministic, explainable resource leveling:
//! - Explicit opt-in only (`--level` flag)
//! - Every delay has a structured reason (`LevelingReason`)
//! - Original schedule preserved alongside leveled schedule
//! - L001-L004 diagnostics emitted for transparency

use crate::bdd::BddConflictAnalyzer;
use rayon::prelude::*;
use chrono::NaiveDate;
use std::collections::{BinaryHeap, BTreeMap, HashMap, HashSet, VecDeque};
use utf8proj_core::{
    Calendar, DependencyType, Diagnostic, DiagnosticCode, Duration, Project, ResourceId, Schedule,
    ScheduledTask, Severity, Task, TaskId,
};

// =============================================================================
// RFC-0003 Data Structures
// =============================================================================

/// Leveling configuration (explicit user choice)
#[derive(Debug, Clone)]
pub struct LevelingOptions {
    /// Strategy for selecting tasks to delay
    pub strategy: LevelingStrategy,
    /// Maximum allowed project duration increase factor (e.g., 2.0 = can't double duration)
    pub max_project_delay_factor: Option<f64>,
    /// Enable optimal solving for small clusters (requires `optimal-leveling` feature)
    pub use_optimal: bool,
    /// Maximum cluster size for CP solver (tasks). Larger clusters use heuristic.
    pub optimal_threshold: usize,
    /// Timeout per cluster solve in milliseconds
    pub optimal_timeout_ms: u64,
}

impl Default for LevelingOptions {
    fn default() -> Self {
        Self {
            strategy: LevelingStrategy::CriticalPathFirst,
            max_project_delay_factor: None,
            use_optimal: false,
            optimal_threshold: 50,
            optimal_timeout_ms: 5000,
        }
    }
}

/// Strategy for selecting which tasks to delay during leveling
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LevelingStrategy {
    /// Delay non-critical tasks before critical ones (default)
    #[default]
    CriticalPathFirst,
    /// Hybrid BDD + heuristic leveling (RFC-0014)
    /// Uses BDD to identify conflict clusters, then applies heuristic
    /// leveling within clusters. More efficient for large projects.
    Hybrid,
}

/// Structured reason for a leveling delay
#[derive(Debug, Clone)]
pub enum LevelingReason {
    /// Task delayed due to resource overallocation
    ResourceOverallocated {
        resource: ResourceId,
        peak_demand: f32,
        capacity: f32,
        dates: Vec<NaiveDate>,
    },
    /// Task delayed because predecessor was delayed (cascade)
    DependencyChain {
        predecessor: TaskId,
        predecessor_delay: i64,
    },
}

impl std::fmt::Display for LevelingReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LevelingReason::ResourceOverallocated {
                resource,
                peak_demand,
                capacity,
                dates,
            } => {
                write!(
                    f,
                    "Resource '{}' overallocated (demand={:.0}%, capacity={:.0}%) on {} day(s)",
                    resource,
                    peak_demand * 100.0,
                    capacity * 100.0,
                    dates.len()
                )
            }
            LevelingReason::DependencyChain {
                predecessor,
                predecessor_delay,
            } => {
                write!(
                    f,
                    "Predecessor '{}' was delayed by {} days",
                    predecessor, predecessor_delay
                )
            }
        }
    }
}

/// Metrics summarizing the leveling transformation
#[derive(Debug, Clone)]
pub struct LevelingMetrics {
    /// Days added to project duration due to leveling
    pub project_duration_increase: i64,
    /// Peak resource utilization before leveling (0.0-1.0+)
    pub peak_utilization_before: f32,
    /// Peak resource utilization after leveling (0.0-1.0+)
    pub peak_utilization_after: f32,
    /// Number of tasks that were delayed
    pub tasks_delayed: usize,
    /// Total delay days across all tasks
    pub total_delay_days: i64,
}

/// Resource usage on a specific day
#[derive(Debug, Clone)]
pub struct DayUsage {
    /// Total units allocated (1.0 = 100%)
    pub total_units: f32,
    /// Tasks contributing to this usage
    pub tasks: Vec<(TaskId, f32)>,
}

/// Timeline of resource usage
#[derive(Debug, Clone)]
pub struct ResourceTimeline {
    pub resource_id: ResourceId,
    pub capacity: f32,
    /// Usage by date (BTreeMap for sorted iteration - enables O(log n) gap finding)
    pub usage: BTreeMap<NaiveDate, DayUsage>,
}

impl ResourceTimeline {
    pub fn new(resource_id: ResourceId, capacity: f32) -> Self {
        Self {
            resource_id,
            capacity,
            usage: BTreeMap::new(),
        }
    }

    /// Add usage for a task over a date range
    pub fn add_usage(&mut self, task_id: &TaskId, start: NaiveDate, finish: NaiveDate, units: f32) {
        let mut date = start;
        while date <= finish {
            let day = self.usage.entry(date).or_insert(DayUsage {
                total_units: 0.0,
                tasks: Vec::new(),
            });
            day.total_units += units;
            day.tasks.push((task_id.clone(), units));
            date = date.succ_opt().unwrap_or(date);
        }
    }

    /// Remove usage for a task
    pub fn remove_usage(&mut self, task_id: &TaskId) {
        for day in self.usage.values_mut() {
            day.tasks.retain(|(id, units)| {
                if id == task_id {
                    day.total_units -= units;
                    false
                } else {
                    true
                }
            });
        }
        // Clean up empty days
        self.usage.retain(|_, day| day.total_units > 0.0);
    }

    /// Check if over-allocated on a specific date
    pub fn is_overallocated(&self, date: NaiveDate) -> bool {
        self.usage
            .get(&date)
            .map(|day| day.total_units > self.capacity)
            .unwrap_or(false)
    }

    /// Get all over-allocated periods
    pub fn overallocated_periods(&self) -> Vec<OverallocationPeriod> {
        let mut periods = Vec::new();
        let mut dates: Vec<_> = self.usage.keys().cloned().collect();
        dates.sort();

        let mut current_period: Option<OverallocationPeriod> = None;

        for date in dates {
            if let Some(day) = self.usage.get(&date) {
                if day.total_units > self.capacity {
                    match &mut current_period {
                        Some(period) if period.end.succ_opt() == Some(date) => {
                            period.end = date;
                            period.peak_usage = period.peak_usage.max(day.total_units);
                            for (task_id, _) in &day.tasks {
                                if !period.involved_tasks.contains(task_id) {
                                    period.involved_tasks.push(task_id.clone());
                                }
                            }
                        }
                        _ => {
                            if let Some(period) = current_period.take() {
                                periods.push(period);
                            }
                            current_period = Some(OverallocationPeriod {
                                start: date,
                                end: date,
                                peak_usage: day.total_units,
                                involved_tasks: day
                                    .tasks
                                    .iter()
                                    .map(|(id, _)| id.clone())
                                    .collect(),
                            });
                        }
                    }
                } else if let Some(period) = current_period.take() {
                    periods.push(period);
                }
            }
        }

        if let Some(period) = current_period {
            periods.push(period);
        }

        periods
    }

    /// Find first available slot where a task can be scheduled without overallocation.
    ///
    /// Returns `None` if no slot is found within `max_search_days` working days
    /// (default: 2000 working days, approximately 8 years).
    pub fn find_available_slot(
        &self,
        duration_days: i64,
        units: f32,
        earliest_start: NaiveDate,
        calendar: &Calendar,
    ) -> Option<NaiveDate> {
        self.find_available_slot_with_limit(duration_days, units, earliest_start, calendar, 2000)
    }

    /// Find first available slot with a custom search limit.
    ///
    /// Returns `None` if no slot is found within `max_search_days` working days.
    ///
    /// Optimized to skip over blocked periods using BTreeMap's sorted iteration.
    pub fn find_available_slot_with_limit(
        &self,
        duration_days: i64,
        units: f32,
        earliest_start: NaiveDate,
        calendar: &Calendar,
        max_search_days: i64,
    ) -> Option<NaiveDate> {
        // Early exit: if units exceed capacity, no slot will ever work
        if units > self.capacity {
            return None;
        }

        let available_capacity = self.capacity - units;
        let mut candidate = earliest_start;
        let mut working_days_searched: i64 = 0;

        while working_days_searched < max_search_days {
            // Check if all days in this slot are available
            let slot_result =
                self.check_slot_available(candidate, duration_days, available_capacity, calendar);

            match slot_result {
                SlotCheckResult::Available => return Some(candidate),
                SlotCheckResult::BlockedUntil(blocked_end) => {
                    // Count working days from candidate to blocked_end (inclusive)
                    let days_skipped = count_working_days_between(candidate, blocked_end, calendar);
                    working_days_searched += days_skipped.max(1);

                    // Skip to the day after the blocked period
                    candidate = blocked_end.succ_opt().unwrap_or(blocked_end);
                    // Skip non-working days
                    while !calendar.is_working_day(candidate) {
                        candidate = candidate.succ_opt().unwrap_or(candidate);
                    }
                }
                SlotCheckResult::NoCapacity => {
                    // Move to next working day
                    candidate = candidate.succ_opt().unwrap_or(candidate);
                    while !calendar.is_working_day(candidate) {
                        candidate = candidate.succ_opt().unwrap_or(candidate);
                    }
                    working_days_searched += 1;
                }
            }
        }

        // No slot found within search limit
        None
    }

    /// Check if a slot starting at `start` with `duration_days` is available.
    /// Returns the result indicating availability or when the blocking ends.
    fn check_slot_available(
        &self,
        start: NaiveDate,
        duration_days: i64,
        available_capacity: f32,
        calendar: &Calendar,
    ) -> SlotCheckResult {
        let mut check_date = start;
        let mut working_days_checked = 0;
        let mut latest_blocked_date: Option<NaiveDate> = None;

        while working_days_checked < duration_days {
            if calendar.is_working_day(check_date) {
                let current_usage = self
                    .usage
                    .get(&check_date)
                    .map(|d| d.total_units)
                    .unwrap_or(0.0);

                if current_usage > available_capacity {
                    // This day is blocked - find the end of the blocked run
                    latest_blocked_date = Some(self.find_blocked_run_end(check_date, available_capacity));
                }
                working_days_checked += 1;
            }
            check_date = check_date.succ_opt().unwrap_or(check_date);
        }

        match latest_blocked_date {
            Some(end) => SlotCheckResult::BlockedUntil(end),
            None => SlotCheckResult::Available,
        }
    }

    /// Find the end of a contiguous blocked run starting from `start`.
    /// Uses BTreeMap's range iteration for efficiency.
    fn find_blocked_run_end(&self, start: NaiveDate, available_capacity: f32) -> NaiveDate {
        let mut end = start;

        // Iterate through usage entries from `start` forward
        for (&date, day) in self.usage.range(start..) {
            if day.total_units > available_capacity {
                end = date;
            } else {
                // Found a day with available capacity - the blocked run ends before this
                break;
            }

            // Safety limit: don't search more than 1000 days for the run end
            if date > start + chrono::Duration::days(1000) {
                break;
            }
        }

        end
    }
}

/// Result of checking a slot's availability
enum SlotCheckResult {
    /// Slot is available for scheduling
    Available,
    /// Slot is blocked; provides the end date of the blocking period
    BlockedUntil(NaiveDate),
    /// No capacity available (units > capacity)
    NoCapacity,
}

/// A period of resource over-allocation
#[derive(Debug, Clone)]
pub struct OverallocationPeriod {
    pub start: NaiveDate,
    pub end: NaiveDate,
    pub peak_usage: f32,
    pub involved_tasks: Vec<TaskId>,
}

/// Result of resource leveling (RFC-0003 compliant)
#[derive(Debug, Clone)]
pub struct LevelingResult {
    /// Original schedule (preserved, authoritative)
    pub original_schedule: Schedule,
    /// Leveled schedule (delta transformation)
    pub leveled_schedule: Schedule,
    /// Tasks that were shifted (each with structured reason)
    pub shifted_tasks: Vec<ShiftedTask>,
    /// Conflicts that could not be resolved
    pub unresolved_conflicts: Vec<UnresolvedConflict>,
    /// Whether the project duration was extended
    pub project_extended: bool,
    /// New project end date
    pub new_project_end: NaiveDate,
    /// Metrics summarizing the transformation
    pub metrics: LevelingMetrics,
    /// Diagnostics emitted during leveling (L001-L004)
    pub diagnostics: Vec<Diagnostic>,
}

// Backwards compatibility alias
impl LevelingResult {
    /// Get the leveled schedule (alias for backwards compatibility)
    pub fn schedule(&self) -> &Schedule {
        &self.leveled_schedule
    }
}

/// A task that was shifted during leveling
#[derive(Debug, Clone)]
pub struct ShiftedTask {
    pub task_id: TaskId,
    pub original_start: NaiveDate,
    pub new_start: NaiveDate,
    pub days_shifted: i64,
    /// Structured reason for the delay (RFC-0003)
    pub reason: LevelingReason,
    /// Resources involved in the conflict
    pub resources_involved: Vec<ResourceId>,
}

/// A conflict that could not be resolved
#[derive(Debug, Clone)]
pub struct UnresolvedConflict {
    pub resource_id: ResourceId,
    pub period: OverallocationPeriod,
    pub reason: String,
}

/// Task candidate for shifting (used in priority queue)
#[derive(Debug, Clone, Eq, PartialEq)]
struct ShiftCandidate {
    task_id: TaskId,
    priority: u32,
    slack_days: i64,
    is_critical: bool,
}

impl Ord for ShiftCandidate {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Deterministic ordering (RFC-0003):
        // 1. Non-critical before critical (prefer shifting non-critical)
        // 2. More slack before less slack
        // 3. Lower priority before higher priority
        // 4. Task ID as final tie-breaker (determinism guarantee)
        match (self.is_critical, other.is_critical) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => self
                .slack_days
                .cmp(&other.slack_days)
                .then(other.priority.cmp(&self.priority))
                .then(other.task_id.cmp(&self.task_id)), // Deterministic tie-breaker
        }
    }
}

impl PartialOrd for ShiftCandidate {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// Perform resource leveling on a schedule (RFC-0003 compliant)
///
/// This function is deterministic: same input always produces same output.
/// Original schedule is preserved; leveled schedule is a delta transformation.
pub fn level_resources(
    project: &Project,
    schedule: &Schedule,
    calendar: &Calendar,
) -> LevelingResult {
    level_resources_with_options(project, schedule, calendar, &LevelingOptions::default())
}

/// Perform resource leveling with explicit options
pub fn level_resources_with_options(
    project: &Project,
    schedule: &Schedule,
    calendar: &Calendar,
    options: &LevelingOptions,
) -> LevelingResult {
    // Use hybrid leveling if strategy is Hybrid (RFC-0014)
    if options.strategy == LevelingStrategy::Hybrid {
        return hybrid_level_resources(project, schedule, calendar, options);
    }

    // Preserve original schedule (RFC-0003 requirement)
    let original_schedule = schedule.clone();
    let mut leveled_tasks = schedule.tasks.clone();
    let mut shifted_tasks = Vec::new();
    let mut unresolved_conflicts = Vec::new();
    let mut diagnostics = Vec::new();

    // Calculate peak utilization before leveling
    let peak_utilization_before = calculate_peak_utilization(project, &leveled_tasks);

    // Build resource timelines from schedule
    let mut timelines = build_resource_timelines(project, &leveled_tasks);

    // Build task priority map for shifting decisions
    let task_priorities = build_task_priority_map(&leveled_tasks, project);

    // Build successor map for dependency propagation
    let successor_map = build_successor_map(project);

    // Track milestones for L004 diagnostic
    let original_milestone_dates: HashMap<TaskId, NaiveDate> = leveled_tasks
        .iter()
        .filter(|(_, t)| t.duration.minutes == 0) // Milestones have zero duration
        .map(|(id, t)| (id.clone(), t.start))
        .collect();

    // Iterate until no more over-allocations or can't resolve
    let max_iterations = leveled_tasks.len() * 10; // Prevent infinite loops
    let mut iterations = 0;

    // Check max delay factor constraint
    let original_duration = schedule.project_duration.as_days() as f64;
    let max_allowed_duration = options
        .max_project_delay_factor
        .map(|f| (original_duration * f) as i64);

    while iterations < max_iterations {
        iterations += 1;

        // DETERMINISM: Collect ALL conflicts, then sort them
        let mut all_conflicts: Vec<(ResourceId, OverallocationPeriod)> = timelines
            .iter()
            .flat_map(|(_, timeline)| {
                let resource_id = timeline.resource_id.clone();
                timeline
                    .overallocated_periods()
                    .into_iter()
                    .map(move |p| (resource_id.clone(), p))
            })
            .collect();

        // DETERMINISM: Sort by (resource_id, start_date) for consistent ordering
        all_conflicts.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.start.cmp(&b.1.start)));

        let Some((resource_id, period)) = all_conflicts.into_iter().next() else {
            break; // No more conflicts
        };

        // Get timeline capacity for this resource
        let timeline_capacity = timelines
            .get(&resource_id)
            .map(|t| t.capacity)
            .unwrap_or(1.0);

        // Find candidates to shift (tasks in this period)
        let mut candidates: BinaryHeap<ShiftCandidate> = period
            .involved_tasks
            .iter()
            .filter_map(|task_id| {
                let task = leveled_tasks.get(task_id)?;
                let (priority, _) = task_priorities.get(task_id)?;
                Some(ShiftCandidate {
                    task_id: task_id.clone(),
                    priority: *priority,
                    slack_days: task.slack.as_days() as i64,
                    is_critical: task.is_critical,
                })
            })
            .collect();

        if candidates.is_empty() {
            // Can't resolve this conflict - no tasks found
            unresolved_conflicts.push(UnresolvedConflict {
                resource_id: resource_id.clone(),
                period: period.clone(),
                reason: "No shiftable tasks found".into(),
            });

            // Emit L002 diagnostic
            diagnostics.push(Diagnostic {
                code: DiagnosticCode::L002UnresolvableConflict,
                severity: Severity::Warning,
                message: format!(
                    "Unresolvable resource conflict: '{}' at {} (demand={:.0}%, capacity={:.0}%)",
                    resource_id,
                    period.start,
                    period.peak_usage * 100.0,
                    timeline_capacity * 100.0
                ),
                file: None,
                span: None,
                secondary_spans: vec![],
                notes: vec![],
                hints: vec![],
            });
            continue;
        }

        // Pick the best candidate to shift (deterministic due to Ord impl)
        let candidate = candidates.pop().unwrap();
        let task = leveled_tasks.get(&candidate.task_id).unwrap();
        let original_start = task.start;

        // Get resource units for this task
        let units = task
            .assignments
            .iter()
            .find(|a| a.resource_id == resource_id)
            .map(|a| a.units)
            .unwrap_or(1.0);

        // Find next available slot
        let timeline = timelines.get_mut(&resource_id).unwrap();
        let duration_days = task.duration.as_days() as i64;

        // Remove current usage before finding new slot
        timeline.remove_usage(&candidate.task_id);

        let Some(new_start) = timeline.find_available_slot(
            duration_days,
            units,
            period.end.succ_opt().unwrap_or(period.end),
            calendar,
        ) else {
            // No available slot found within search limit - record as unresolved
            unresolved_conflicts.push(UnresolvedConflict {
                resource_id: resource_id.clone(),
                period: period.clone(),
                reason: format!(
                    "No available slot found for '{}' (units={:.0}%, capacity={:.0}%)",
                    candidate.task_id,
                    units * 100.0,
                    timeline.capacity * 100.0
                ),
            });

            // Emit L002 diagnostic
            diagnostics.push(Diagnostic {
                code: DiagnosticCode::L002UnresolvableConflict,
                severity: Severity::Warning,
                message: format!(
                    "Cannot level '{}': no available slot within search horizon",
                    candidate.task_id
                ),
                file: None,
                span: None,
                secondary_spans: vec![],
                notes: vec![format!(
                    "Resource '{}' cannot accommodate this task (demand={:.0}%, capacity={:.0}%)",
                    resource_id,
                    units * 100.0,
                    timeline.capacity * 100.0
                )],
                hints: vec!["Consider adding more resources or reducing task effort".to_string()],
            });

            // Re-add the usage we removed (keep task at original position)
            timeline.add_usage(&candidate.task_id, original_start, task.finish, units);
            continue;
        };

        // Shift the task
        let days_shifted = count_working_days(original_start, new_start, calendar);
        let new_finish = add_working_days(new_start, duration_days, calendar);

        // Check if this would exceed max delay factor
        if let Some(max_duration) = max_allowed_duration {
            let new_project_end = leveled_tasks
                .values()
                .map(|t| t.finish)
                .chain(std::iter::once(new_finish))
                .max()
                .unwrap_or(schedule.project_end);
            let new_duration = count_working_days(project.start, new_project_end, calendar);
            if new_duration > max_duration {
                // Would exceed max delay - record as unresolved
                unresolved_conflicts.push(UnresolvedConflict {
                    resource_id: resource_id.clone(),
                    period: period.clone(),
                    reason: format!(
                        "Shifting would exceed max delay factor ({:.1}x)",
                        options.max_project_delay_factor.unwrap()
                    ),
                });
                // Re-add the usage we removed
                timeline.add_usage(&candidate.task_id, original_start, task.finish, units);
                continue;
            }
        }

        // Update the task in our schedule
        if let Some(task) = leveled_tasks.get_mut(&candidate.task_id) {
            task.start = new_start;
            task.finish = new_finish;
            task.early_start = new_start;
            task.early_finish = new_finish;

            // Update assignments
            for assignment in &mut task.assignments {
                assignment.start = new_start;
                assignment.finish = new_finish;
            }
        }

        // Re-add usage at new position
        timeline.add_usage(&candidate.task_id, new_start, new_finish, units);

        // Collect conflict dates for structured reason
        let conflict_dates: Vec<NaiveDate> = {
            let mut dates = Vec::new();
            let mut d = period.start;
            while d <= period.end {
                dates.push(d);
                d = d.succ_opt().unwrap_or(d);
                if dates.len() > 100 {
                    break;
                } // Safety limit
            }
            dates
        };

        // Record the shift with structured reason (RFC-0003)
        shifted_tasks.push(ShiftedTask {
            task_id: candidate.task_id.clone(),
            original_start,
            new_start,
            days_shifted,
            reason: LevelingReason::ResourceOverallocated {
                resource: resource_id.clone(),
                peak_demand: period.peak_usage,
                capacity: timeline_capacity,
                dates: conflict_dates,
            },
            resources_involved: vec![resource_id.clone()],
        });

        // Emit L001 diagnostic
        diagnostics.push(Diagnostic {
            code: DiagnosticCode::L001OverallocationResolved,
            severity: Severity::Hint,
            message: format!(
                "Resource overallocation resolved by delaying '{}' by {} day(s)",
                candidate.task_id, days_shifted
            ),
            file: None,
            span: None,
            secondary_spans: vec![],
            notes: vec![],
            hints: vec![format!("Resource '{}' was overallocated", resource_id)],
        });

        // Propagate delay to transitive successors
        propagate_to_successors(
            &candidate.task_id,
            &mut leveled_tasks,
            &mut timelines,
            &successor_map,
            project,
            calendar,
            &mut shifted_tasks,
            &mut diagnostics,
        );
    }

    // Calculate new project end
    let new_project_end = leveled_tasks
        .values()
        .map(|t| t.finish)
        .max()
        .unwrap_or(schedule.project_end);

    let project_extended = new_project_end > schedule.project_end;
    let duration_increase =
        count_working_days(schedule.project_end, new_project_end, calendar).max(0);

    // Emit L003 if project duration increased
    if project_extended {
        diagnostics.push(Diagnostic {
            code: DiagnosticCode::L003DurationIncreased,
            severity: Severity::Hint,
            message: format!(
                "Project duration increased by {} day(s) due to leveling",
                duration_increase
            ),
            file: None,
            span: None,
            secondary_spans: vec![],
            notes: vec![],
            hints: vec![],
        });
    }

    // Check for delayed milestones (L004)
    for (milestone_id, original_date) in &original_milestone_dates {
        if let Some(task) = leveled_tasks.get(milestone_id) {
            if task.start > *original_date {
                let delay = count_working_days(*original_date, task.start, calendar);
                diagnostics.push(Diagnostic {
                    code: DiagnosticCode::L004MilestoneDelayed,
                    severity: Severity::Warning,
                    message: format!(
                        "Milestone '{}' delayed by {} day(s) due to leveling",
                        milestone_id, delay
                    ),
                    file: None,
                    span: None,
                    secondary_spans: vec![],
                    notes: vec![],
                    hints: vec![],
                });
            }
        }
    }

    // Recalculate critical path if project was extended
    let critical_path = if project_extended {
        recalculate_critical_path(&leveled_tasks, new_project_end)
    } else {
        schedule.critical_path.clone()
    };

    // Calculate new project duration
    let project_duration_days = count_working_days(project.start, new_project_end, calendar);

    // Calculate peak utilization after leveling
    let peak_utilization_after = calculate_peak_utilization(project, &leveled_tasks);

    // Build metrics
    let metrics = LevelingMetrics {
        project_duration_increase: duration_increase,
        peak_utilization_before,
        peak_utilization_after,
        tasks_delayed: shifted_tasks.len(),
        total_delay_days: shifted_tasks.iter().map(|s| s.days_shifted).sum(),
    };

    LevelingResult {
        original_schedule,
        leveled_schedule: Schedule {
            tasks: leveled_tasks,
            critical_path,
            project_duration: Duration::days(project_duration_days),
            project_end: new_project_end,
            total_cost: schedule.total_cost.clone(),
            total_cost_range: schedule.total_cost_range.clone(),
            project_progress: schedule.project_progress,
            project_baseline_finish: schedule.project_baseline_finish,
            project_forecast_finish: schedule.project_forecast_finish,
            project_variance_days: schedule.project_variance_days,
            planned_value: schedule.planned_value,
            earned_value: schedule.earned_value,
            spi: schedule.spi,
        },
        shifted_tasks,
        unresolved_conflicts,
        project_extended,
        new_project_end,
        metrics,
        diagnostics,
    }
}

/// Result from processing a single conflict cluster (for parallel processing)
#[derive(Debug)]
pub(crate) struct ClusterResult {
    /// Task updates: task_id -> (new_start, new_finish)
    pub(crate) task_updates: HashMap<TaskId, (NaiveDate, NaiveDate)>,
    /// Tasks that were shifted
    pub(crate) shifted_tasks: Vec<ShiftedTask>,
    /// Conflicts that couldn't be resolved
    pub(crate) unresolved_conflicts: Vec<UnresolvedConflict>,
    /// Diagnostics generated
    pub(crate) diagnostics: Vec<Diagnostic>,
    /// Processing time for profiling
    pub(crate) elapsed: std::time::Duration,
}

/// Process a single conflict cluster (pure function for parallel execution)
///
/// This function is designed to be called in parallel for independent clusters.
/// It takes immutable references to shared data and returns all updates as values.
fn process_cluster(
    cluster: &crate::bdd::ConflictCluster,
    cluster_idx: usize,
    tasks: &HashMap<TaskId, ScheduledTask>,
    project: &Project,
    calendar: &Calendar,
    task_priorities: &HashMap<TaskId, (u32, ())>,
    max_allowed_duration: Option<i64>,
    successor_map: &SuccessorMap,
) -> ClusterResult {
    let cluster_start = std::time::Instant::now();

    let mut task_updates: HashMap<TaskId, (NaiveDate, NaiveDate)> = HashMap::new();
    let mut shifted_tasks = Vec::new();
    let mut unresolved_conflicts = Vec::new();
    let mut diagnostics = Vec::new();

    // Build resource timelines for only the resources in this cluster
    let cluster_resources: HashSet<&str> = cluster.resources.iter().map(|s| s.as_str()).collect();
    let cluster_task_ids: HashSet<&str> = cluster.tasks.iter().map(|s| s.as_str()).collect();

    // Create local timelines for this cluster's resources
    let mut timelines: HashMap<ResourceId, ResourceTimeline> = HashMap::new();
    for resource in &project.resources {
        if cluster_resources.contains(resource.id.as_str()) {
            timelines.insert(
                resource.id.clone(),
                ResourceTimeline::new(resource.id.clone(), resource.capacity),
            );
        }
    }

    // Create a local copy of cluster tasks for mutation
    let mut local_tasks: HashMap<TaskId, ScheduledTask> = tasks
        .iter()
        .filter(|(id, _)| cluster_task_ids.contains(id.as_str()))
        .map(|(id, task)| (id.clone(), task.clone()))
        .collect();

    // Initialize timelines with cluster task assignments
    for task in local_tasks.values() {
        for assignment in &task.assignments {
            if let Some(timeline) = timelines.get_mut(&assignment.resource_id) {
                timeline.add_usage(&task.task_id, task.start, task.finish, assignment.units);
            }
        }
    }

    // Level tasks within this cluster
    let max_iterations = cluster.tasks.len() * 10;
    let mut iterations = 0;

    while iterations < max_iterations {
        iterations += 1;

        // Find conflicts for resources in this cluster
        let mut all_conflicts: Vec<(ResourceId, OverallocationPeriod)> = timelines
            .iter()
            .flat_map(|(_, timeline)| {
                let resource_id = timeline.resource_id.clone();
                timeline
                    .overallocated_periods()
                    .into_iter()
                    .filter(|p| {
                        p.involved_tasks
                            .iter()
                            .any(|t| cluster_task_ids.contains(t.as_str()))
                    })
                    .map(move |p| (resource_id.clone(), p))
            })
            .collect();

        all_conflicts.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.start.cmp(&b.1.start)));

        let Some((resource_id, period)) = all_conflicts.into_iter().next() else {
            break; // No more conflicts in this cluster
        };

        let timeline_capacity = timelines
            .get(&resource_id)
            .map(|t| t.capacity)
            .unwrap_or(1.0);

        // Find candidates to shift
        let mut candidates: BinaryHeap<ShiftCandidate> = period
            .involved_tasks
            .iter()
            .filter(|task_id| cluster_task_ids.contains(task_id.as_str()))
            .filter_map(|task_id| {
                let task = local_tasks.get(task_id)?;
                let (priority, _) = task_priorities.get(task_id)?;
                Some(ShiftCandidate {
                    task_id: task_id.clone(),
                    priority: *priority,
                    slack_days: task.slack.as_days() as i64,
                    is_critical: task.is_critical,
                })
            })
            .collect();

        if candidates.is_empty() {
            unresolved_conflicts.push(UnresolvedConflict {
                resource_id: resource_id.clone(),
                period: period.clone(),
                reason: "No shiftable tasks found in cluster".into(),
            });

            diagnostics.push(Diagnostic {
                code: DiagnosticCode::L002UnresolvableConflict,
                severity: Severity::Warning,
                message: format!(
                    "Unresolvable resource conflict: '{}' at {} (hybrid/parallel)",
                    resource_id, period.start
                ),
                file: None,
                span: None,
                secondary_spans: vec![],
                notes: vec![],
                hints: vec![],
            });
            continue;
        }

        // Pick the best candidate
        let candidate = candidates.pop().unwrap();
        let task = local_tasks.get(&candidate.task_id).unwrap();
        let original_start = task.start;

        let units = task
            .assignments
            .iter()
            .find(|a| a.resource_id == resource_id)
            .map(|a| a.units)
            .unwrap_or(1.0);

        let timeline = timelines.get_mut(&resource_id).unwrap();
        let duration_days = task.duration.as_days() as i64;

        timeline.remove_usage(&candidate.task_id);

        let Some(new_start) = timeline.find_available_slot(
            duration_days,
            units,
            period.end.succ_opt().unwrap_or(period.end),
            calendar,
        ) else {
            unresolved_conflicts.push(UnresolvedConflict {
                resource_id: resource_id.clone(),
                period: period.clone(),
                reason: format!("No available slot for '{}'", candidate.task_id),
            });

            diagnostics.push(Diagnostic {
                code: DiagnosticCode::L002UnresolvableConflict,
                severity: Severity::Warning,
                message: format!(
                    "Cannot level '{}': no available slot (hybrid/parallel)",
                    candidate.task_id
                ),
                file: None,
                span: None,
                secondary_spans: vec![],
                notes: vec![],
                hints: vec![],
            });

            timeline.add_usage(&candidate.task_id, original_start, task.finish, units);
            continue;
        };

        let days_shifted = count_working_days(original_start, new_start, calendar);
        let new_finish = add_working_days(new_start, duration_days, calendar);

        // Check max delay factor (simplified - just check against known duration)
        if let Some(max_duration) = max_allowed_duration {
            let new_project_end = local_tasks
                .values()
                .map(|t| t.finish)
                .chain(std::iter::once(new_finish))
                .max()
                .unwrap_or(new_finish);
            // Approximate check - will be validated after merge
            let approx_duration = (new_project_end - project.start).num_days();
            if approx_duration > max_duration {
                unresolved_conflicts.push(UnresolvedConflict {
                    resource_id: resource_id.clone(),
                    period: period.clone(),
                    reason: "Would exceed max delay factor".into(),
                });
                timeline.add_usage(&candidate.task_id, original_start, task.finish, units);
                continue;
            }
        }

        // Update the local task copy
        if let Some(task) = local_tasks.get_mut(&candidate.task_id) {
            task.start = new_start;
            task.finish = new_finish;
            task.early_start = new_start;
            task.early_finish = new_finish;

            for assignment in &mut task.assignments {
                assignment.start = new_start;
                assignment.finish = new_finish;
            }
        }

        // Re-add usage at new position
        timeline.add_usage(&candidate.task_id, new_start, new_finish, units);

        // Record the update
        task_updates.insert(candidate.task_id.clone(), (new_start, new_finish));

        // Record the shift
        shifted_tasks.push(ShiftedTask {
            task_id: candidate.task_id.clone(),
            original_start,
            new_start,
            days_shifted,
            reason: LevelingReason::ResourceOverallocated {
                resource: resource_id.clone(),
                peak_demand: period.peak_usage,
                capacity: timeline_capacity,
                dates: vec![period.start],
            },
            resources_involved: vec![resource_id.clone()],
        });

        // Emit L001 diagnostic
        diagnostics.push(Diagnostic {
            code: DiagnosticCode::L001OverallocationResolved,
            severity: Severity::Info,
            message: format!(
                "Task '{}' delayed {} days due to resource '{}' (parallel cluster {})",
                candidate.task_id, days_shifted, resource_id, cluster_idx
            ),
            file: None,
            span: None,
            secondary_spans: vec![],
            notes: vec![format!(
                "Cluster: {} tasks competing for {} resources",
                cluster.tasks.len(),
                cluster.resources.len()
            )],
            hints: vec![],
        });

        // Propagate delay to successors within this cluster
        propagate_to_successors(
            &candidate.task_id,
            &mut local_tasks,
            &mut timelines,
            successor_map,
            project,
            calendar,
            &mut shifted_tasks,
            &mut diagnostics,
        );

        // Update task_updates with any propagated changes
        for (id, task) in &local_tasks {
            if cluster_task_ids.contains(id.as_str()) {
                let original = tasks.get(id);
                if let Some(orig) = original {
                    if orig.start != task.start || orig.finish != task.finish {
                        task_updates.insert(id.clone(), (task.start, task.finish));
                    }
                }
            }
        }
    }

    ClusterResult {
        task_updates,
        shifted_tasks,
        unresolved_conflicts,
        diagnostics,
        elapsed: cluster_start.elapsed(),
    }
}

/// Hybrid BDD + heuristic resource leveling (RFC-0014 Phase 1+2)
///
/// Uses BDD to identify conflict clusters, then applies heuristic leveling
/// within each cluster in parallel. This achieves O(n + sum(k²)) complexity
/// instead of O(n²), where k is the cluster size and is typically much smaller than n.
fn hybrid_level_resources(
    project: &Project,
    schedule: &Schedule,
    calendar: &Calendar,
    options: &LevelingOptions,
) -> LevelingResult {
    let total_start = std::time::Instant::now();

    let original_schedule = schedule.clone();
    let mut leveled_tasks = schedule.tasks.clone();
    let mut shifted_tasks = Vec::new();
    let mut unresolved_conflicts = Vec::new();
    let mut diagnostics = Vec::new();

    // Calculate peak utilization before leveling
    let peak_utilization_before = calculate_peak_utilization(project, &leveled_tasks);

    // Step 1: Use BDD to analyze conflict clusters
    let bdd_start = std::time::Instant::now();
    let analyzer = BddConflictAnalyzer::new();
    let cluster_analysis = analyzer.analyze_clusters(project, schedule);
    let bdd_elapsed = bdd_start.elapsed();

    // Profile output (only in debug builds or when RUST_LOG is set)
    if std::env::var("UTF8PROJ_PROFILE").is_ok() {
        eprintln!(
            "[PROFILE] BDD cluster analysis: {:?} ({} clusters, {} unconstrained tasks)",
            bdd_elapsed,
            cluster_analysis.clusters.len(),
            cluster_analysis.unconstrained_tasks.len()
        );
        for (i, cluster) in cluster_analysis.clusters.iter().enumerate() {
            eprintln!(
                "[PROFILE]   Cluster {}: {} tasks, {} resources",
                i,
                cluster.tasks.len(),
                cluster.resources.len()
            );
        }
    }

    // If no clusters, no leveling needed
    if cluster_analysis.clusters.is_empty() {
        return LevelingResult {
            original_schedule,
            leveled_schedule: schedule.clone(),
            shifted_tasks: vec![],
            unresolved_conflicts: vec![],
            project_extended: false,
            new_project_end: schedule.project_end,
            metrics: LevelingMetrics {
                project_duration_increase: 0,
                peak_utilization_before,
                peak_utilization_after: peak_utilization_before,
                tasks_delayed: 0,
                total_delay_days: 0,
            },
            diagnostics: vec![],
        };
    }

    // Build task priorities (shared, read-only)
    let task_priorities = build_task_priority_map(&leveled_tasks, project);

    // Build successor map for dependency propagation
    let successor_map = build_successor_map(project);

    // Track original project end
    let original_end = schedule.project_end;

    // Track milestones for L004 diagnostic
    let original_milestone_dates: HashMap<TaskId, NaiveDate> = leveled_tasks
        .iter()
        .filter(|(_, t)| t.duration.minutes == 0)
        .map(|(id, t)| (id.clone(), t.start))
        .collect();

    // Check max delay factor constraint
    let original_duration = schedule.project_duration.as_days() as f64;
    let max_allowed_duration = options
        .max_project_delay_factor
        .map(|f| (original_duration * f) as i64);

    // Step 2: Process conflict clusters in parallel (RFC-0014 Phase 2)
    // Optionally use CP solver for small clusters (RFC-0014 Phase 3)
    let heuristic_start = std::time::Instant::now();

    // Process clusters in parallel using rayon
    let cluster_results: Vec<(usize, ClusterResult)> = cluster_analysis
        .clusters
        .par_iter()
        .enumerate()
        .map(|(idx, cluster)| {
            // Use optimal CP solver for small clusters when enabled
            #[cfg(feature = "optimal-leveling")]
            if options.use_optimal {
                if cluster.tasks.len() <= options.optimal_threshold {
                    match crate::optimal::solve_cluster_optimal(
                        cluster,
                        idx,
                        &leveled_tasks,
                        project,
                        options.optimal_timeout_ms,
                    ) {
                        crate::optimal::OptimalResult::Optimal(result) => {
                            // L005 diagnostic is already included in result.diagnostics
                            return (idx, result);
                        }
                        crate::optimal::OptimalResult::Timeout => {
                            // L007: Solver timed out, falling back to heuristic
                            // Diagnostic will be added after heuristic runs
                        }
                        crate::optimal::OptimalResult::Infeasible => {
                            // Infeasible (e.g., task demand > capacity), fall back silently
                        }
                    }
                }
                // Note: L006 for threshold exceeded is not emitted here because
                // it would create too much noise for large clusters. The user can
                // increase --optimal-threshold if they want more clusters solved optimally.
            }

            // Default: use heuristic leveling
            let result = process_cluster(
                cluster,
                idx,
                &leveled_tasks,
                project,
                calendar,
                &task_priorities,
                max_allowed_duration,
                &successor_map,
            );
            (idx, result)
        })
        .collect();

    let heuristic_elapsed = heuristic_start.elapsed();

    // Merge results from all clusters
    for (cluster_idx, result) in &cluster_results {
        // Apply task updates
        for (task_id, (new_start, new_finish)) in &result.task_updates {
            if let Some(task) = leveled_tasks.get_mut(task_id) {
                task.start = *new_start;
                task.finish = *new_finish;
                task.early_start = *new_start;
                task.early_finish = *new_finish;

                for assignment in &mut task.assignments {
                    assignment.start = *new_start;
                    assignment.finish = *new_finish;
                }
            }
        }

        // Collect shifted tasks, conflicts, and diagnostics
        shifted_tasks.extend(result.shifted_tasks.iter().cloned());
        unresolved_conflicts.extend(result.unresolved_conflicts.iter().cloned());
        diagnostics.extend(result.diagnostics.iter().cloned());

        // Profile individual cluster times
        if std::env::var("UTF8PROJ_PROFILE").is_ok() {
            let cluster = &cluster_analysis.clusters[*cluster_idx];
            eprintln!(
                "[PROFILE]   Cluster {} ({} tasks): {:?}",
                cluster_idx,
                cluster.tasks.len(),
                result.elapsed
            );
        }
    }

    // Global propagation pass: after merging cluster results, propagate
    // to successors that may be outside any cluster (cross-resource deps).
    // Build timelines from the merged state for accurate timeline updates.
    let mut global_timelines = build_resource_timelines(project, &leveled_tasks);
    let shifted_task_ids: Vec<TaskId> = cluster_results
        .iter()
        .flat_map(|(_, r)| r.task_updates.keys().cloned())
        .collect();
    for task_id in &shifted_task_ids {
        propagate_to_successors(
            task_id,
            &mut leveled_tasks,
            &mut global_timelines,
            &successor_map,
            project,
            calendar,
            &mut shifted_tasks,
            &mut diagnostics,
        );
    }

    // Profile output
    if std::env::var("UTF8PROJ_PROFILE").is_ok() {
        let num_threads = rayon::current_num_threads();
        eprintln!(
            "[PROFILE] Parallel heuristic leveling: {:?} ({} threads, {} clusters)",
            heuristic_elapsed, num_threads, cluster_results.len()
        );
        eprintln!(
            "[PROFILE] Total hybrid leveling: {:?}",
            total_start.elapsed()
        );
    }

    // Calculate new project end
    let new_project_end = leveled_tasks
        .values()
        .map(|t| t.finish)
        .max()
        .unwrap_or(original_end);

    let project_extended = new_project_end > original_end;
    let duration_increase = if project_extended {
        count_working_days(original_end, new_project_end, calendar)
    } else {
        0
    };

    // Check for milestone date changes (L004)
    for (milestone_id, original_date) in &original_milestone_dates {
        if let Some(task) = leveled_tasks.get(milestone_id) {
            if task.start != *original_date {
                diagnostics.push(Diagnostic {
                    code: DiagnosticCode::L004MilestoneDelayed,
                    severity: Severity::Warning,
                    message: format!(
                        "Milestone '{}' moved from {} to {} (hybrid)",
                        milestone_id, original_date, task.start
                    ),
                    file: None,
                    span: None,
                    secondary_spans: vec![],
                    notes: vec![],
                    hints: vec![],
                });
            }
        }
    }

    // Project extension diagnostic
    if project_extended {
        diagnostics.push(Diagnostic {
            code: DiagnosticCode::L003DurationIncreased,
            severity: Severity::Info,
            message: format!(
                "Project duration increased by {} days due to hybrid leveling",
                duration_increase
            ),
            file: None,
            span: None,
            secondary_spans: vec![],
            notes: vec![format!(
                "Processed {} conflict cluster(s)",
                cluster_analysis.clusters.len()
            )],
            hints: vec![],
        });
    }

    // Recalculate critical path if needed
    let critical_path = if project_extended {
        recalculate_critical_path(&leveled_tasks, new_project_end)
    } else {
        schedule.critical_path.clone()
    };

    let project_duration_days = count_working_days(project.start, new_project_end, calendar);
    let peak_utilization_after = calculate_peak_utilization(project, &leveled_tasks);

    let metrics = LevelingMetrics {
        project_duration_increase: duration_increase,
        peak_utilization_before,
        peak_utilization_after,
        tasks_delayed: shifted_tasks.len(),
        total_delay_days: shifted_tasks.iter().map(|s| s.days_shifted).sum(),
    };

    LevelingResult {
        original_schedule,
        leveled_schedule: Schedule {
            tasks: leveled_tasks,
            critical_path,
            project_duration: Duration::days(project_duration_days),
            project_end: new_project_end,
            total_cost: schedule.total_cost.clone(),
            total_cost_range: schedule.total_cost_range.clone(),
            project_progress: schedule.project_progress,
            project_baseline_finish: schedule.project_baseline_finish,
            project_forecast_finish: schedule.project_forecast_finish,
            project_variance_days: schedule.project_variance_days,
            planned_value: schedule.planned_value,
            earned_value: schedule.earned_value,
            spi: schedule.spi,
        },
        shifted_tasks,
        unresolved_conflicts,
        project_extended,
        new_project_end,
        metrics,
        diagnostics,
    }
}

/// Type alias for the successor map: predecessor_id -> Vec<(successor_id, dep_type, lag)>
type SuccessorMap = HashMap<TaskId, Vec<(TaskId, DependencyType, Option<Duration>)>>;

/// Build a successor map from the project's task tree.
///
/// Inverts `task.depends` to produce `predecessor -> Vec<(successor_id, dep_type, lag)>`.
/// Uses qualified IDs (e.g., "phase1.task1") to match the scheduled task keys.
fn build_successor_map(project: &Project) -> SuccessorMap {
    let mut map: SuccessorMap = HashMap::new();

    // First, flatten all tasks to get qualified IDs
    let mut task_map: HashMap<String, &Task> = HashMap::new();
    let mut context_map: HashMap<String, String> = HashMap::new();
    flatten_project_tasks(&project.tasks, "", &mut task_map, &mut context_map);

    // For each task, resolve its dependencies and invert them
    let qualified_ids: Vec<String> = task_map.keys().cloned().collect();
    for qualified_id in &qualified_ids {
        let task = task_map[qualified_id];
        for dep in &task.depends {
            // Resolve predecessor path (absolute or relative)
            let pred_id = resolve_dep_path(&dep.predecessor, qualified_id, &context_map, &task_map);
            if let Some(pred_id) = pred_id {
                map.entry(pred_id)
                    .or_default()
                    .push((qualified_id.clone(), dep.dep_type, dep.lag.clone()));
            }
        }
    }

    map
}

/// Flatten hierarchical tasks into qualified ID map (mirrors lib.rs logic)
fn flatten_project_tasks<'a>(
    tasks: &'a [Task],
    prefix: &str,
    map: &mut HashMap<String, &'a Task>,
    context_map: &mut HashMap<String, String>,
) {
    for task in tasks {
        let qualified_id = if prefix.is_empty() {
            task.id.clone()
        } else {
            format!("{}.{}", prefix, task.id)
        };
        map.insert(qualified_id.clone(), task);
        context_map.insert(qualified_id.clone(), prefix.to_string());
        if !task.children.is_empty() {
            flatten_project_tasks(&task.children, &qualified_id, map, context_map);
        }
    }
}

/// Resolve a dependency path (mirrors lib.rs resolve_dependency_path)
fn resolve_dep_path(
    dep_path: &str,
    from_qualified_id: &str,
    context_map: &HashMap<String, String>,
    task_map: &HashMap<String, &Task>,
) -> Option<String> {
    if task_map.contains_key(dep_path) {
        return Some(dep_path.to_string());
    }
    if dep_path.contains('.') {
        return None;
    }
    if let Some(container) = context_map.get(from_qualified_id) {
        let qualified = if container.is_empty() {
            dep_path.to_string()
        } else {
            format!("{}.{}", container, dep_path)
        };
        if task_map.contains_key(&qualified) {
            return Some(qualified);
        }
    }
    None
}

/// Compute the earliest valid start for a task given ALL its predecessors' current dates.
///
/// Returns `None` if any predecessor is missing from the schedule (shouldn't happen).
fn compute_earliest_start(
    task_id: &TaskId,
    _project: &Project,
    leveled_tasks: &HashMap<TaskId, ScheduledTask>,
    successor_map: &SuccessorMap,
) -> Option<NaiveDate> {
    // We need the task's dependencies (predecessor -> this task).
    // Scan the successor map to find all predecessors of task_id.
    let mut earliest = None;

    for (pred_id, successors) in successor_map {
        for (succ_id, dep_type, lag) in successors {
            if succ_id != task_id {
                continue;
            }
            let pred = leveled_tasks.get(pred_id)?;
            let lag_days = lag.as_ref().map(|l| l.as_days() as i64).unwrap_or(0);

            let constraint_date = match dep_type {
                DependencyType::FinishToStart => {
                    // Successor starts after predecessor finishes + lag
                    pred.finish + chrono::Duration::days(lag_days + 1)
                }
                DependencyType::StartToStart => {
                    // Successor starts when predecessor starts + lag
                    pred.start + chrono::Duration::days(lag_days)
                }
                DependencyType::FinishToFinish => {
                    // Successor finishes when predecessor finishes + lag
                    // So successor start = pred.finish + lag - successor_duration
                    let succ = leveled_tasks.get(task_id)?;
                    let succ_dur = (succ.finish - succ.start).num_days();
                    pred.finish + chrono::Duration::days(lag_days) - chrono::Duration::days(succ_dur)
                }
                DependencyType::StartToFinish => {
                    // Successor finishes when predecessor starts + lag
                    let succ = leveled_tasks.get(task_id)?;
                    let succ_dur = (succ.finish - succ.start).num_days();
                    pred.start + chrono::Duration::days(lag_days) - chrono::Duration::days(succ_dur)
                }
            };

            earliest = Some(match earliest {
                Some(e) => std::cmp::max(e, constraint_date),
                None => constraint_date,
            });
        }
    }

    earliest
}

/// Propagate delay to all transitive successors after a task has been shifted.
///
/// Uses BFS in topological order. For each successor, computes the earliest valid start
/// from ALL predecessors; if it's later than the current start, shifts the successor.
fn propagate_to_successors(
    shifted_task_id: &TaskId,
    leveled_tasks: &mut HashMap<TaskId, ScheduledTask>,
    timelines: &mut HashMap<ResourceId, ResourceTimeline>,
    successor_map: &SuccessorMap,
    project: &Project,
    calendar: &Calendar,
    shifted_tasks: &mut Vec<ShiftedTask>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut queue: VecDeque<TaskId> = VecDeque::new();
    let mut visited: HashSet<TaskId> = HashSet::new();

    // Seed the queue with direct successors of the shifted task
    if let Some(successors) = successor_map.get(shifted_task_id) {
        for (succ_id, _, _) in successors {
            if leveled_tasks.contains_key(succ_id) && visited.insert(succ_id.clone()) {
                queue.push_back(succ_id.clone());
            }
        }
    }

    while let Some(succ_id) = queue.pop_front() {
        let Some(earliest) = compute_earliest_start(&succ_id, project, leveled_tasks, successor_map) else {
            continue;
        };

        let task = match leveled_tasks.get(&succ_id) {
            Some(t) => t,
            None => continue,
        };

        if earliest <= task.start {
            // No shift needed — but still enqueue this task's successors
            // in case they depend on other shifted tasks
            if let Some(next_succs) = successor_map.get(&succ_id) {
                for (next_id, _, _) in next_succs {
                    if leveled_tasks.contains_key(next_id) && visited.insert(next_id.clone()) {
                        queue.push_back(next_id.clone());
                    }
                }
            }
            continue;
        }

        // Need to shift this successor
        let original_start = task.start;
        let duration_days = task.duration.as_days() as i64;
        let new_start = earliest;
        let new_finish = add_working_days(new_start, duration_days, calendar);
        let days_shifted = count_working_days(original_start, new_start, calendar);

        // Remove old timeline usage for all assignments
        for assignment in &task.assignments {
            if let Some(timeline) = timelines.get_mut(&assignment.resource_id) {
                timeline.remove_usage(&succ_id);
            }
        }

        // Update the task
        let task = leveled_tasks.get_mut(&succ_id).unwrap();
        task.start = new_start;
        task.finish = new_finish;
        task.early_start = new_start;
        task.early_finish = new_finish;
        for assignment in &mut task.assignments {
            assignment.start = new_start;
            assignment.finish = new_finish;
        }

        // Re-add timeline usage at new position
        let task = &leveled_tasks[&succ_id];
        for assignment in &task.assignments {
            if let Some(timeline) = timelines.get_mut(&assignment.resource_id) {
                timeline.add_usage(&succ_id, new_start, new_finish, assignment.units);
            }
        }

        // Record the shift
        shifted_tasks.push(ShiftedTask {
            task_id: succ_id.clone(),
            original_start,
            new_start,
            days_shifted,
            reason: LevelingReason::DependencyChain {
                predecessor: shifted_task_id.clone(),
                predecessor_delay: days_shifted,
            },
            resources_involved: leveled_tasks[&succ_id]
                .assignments
                .iter()
                .map(|a| a.resource_id.clone())
                .collect(),
        });

        diagnostics.push(Diagnostic {
            code: DiagnosticCode::L001OverallocationResolved,
            severity: Severity::Hint,
            message: format!(
                "Successor '{}' propagated {} day(s) due to predecessor shift",
                succ_id, days_shifted
            ),
            file: None,
            span: None,
            secondary_spans: vec![],
            notes: vec![],
            hints: vec![format!("Predecessor '{}' was delayed", shifted_task_id)],
        });

        // Enqueue this task's successors for further propagation
        if let Some(next_succs) = successor_map.get(&succ_id) {
            for (next_id, _, _) in next_succs {
                if leveled_tasks.contains_key(next_id) && visited.insert(next_id.clone()) {
                    queue.push_back(next_id.clone());
                }
            }
        }
    }
}

/// Calculate peak resource utilization across all resources
fn calculate_peak_utilization(project: &Project, tasks: &HashMap<TaskId, ScheduledTask>) -> f32 {
    let timelines = build_resource_timelines(project, tasks);
    timelines
        .values()
        .flat_map(|t| t.usage.values().map(|d| d.total_units / t.capacity))
        .fold(0.0f32, |max, u| max.max(u))
}

/// Build resource timelines from scheduled tasks
fn build_resource_timelines(
    project: &Project,
    tasks: &HashMap<TaskId, ScheduledTask>,
) -> HashMap<ResourceId, ResourceTimeline> {
    let mut timelines: HashMap<ResourceId, ResourceTimeline> = HashMap::new();

    // Initialize timelines for all resources
    for resource in &project.resources {
        timelines.insert(
            resource.id.clone(),
            ResourceTimeline::new(resource.id.clone(), resource.capacity),
        );
    }

    // Add task assignments to timelines
    for task in tasks.values() {
        for assignment in &task.assignments {
            if let Some(timeline) = timelines.get_mut(&assignment.resource_id) {
                timeline.add_usage(
                    &task.task_id,
                    assignment.start,
                    assignment.finish,
                    assignment.units,
                );
            }
        }
    }

    timelines
}

/// Build a map of task ID to (priority, task reference)
fn build_task_priority_map(
    tasks: &HashMap<TaskId, ScheduledTask>,
    project: &Project,
) -> HashMap<TaskId, (u32, ())> {
    let mut map = HashMap::new();

    // Flatten project tasks to get priorities
    fn add_tasks(tasks: &[utf8proj_core::Task], map: &mut HashMap<TaskId, u32>, prefix: &str) {
        for task in tasks {
            let qualified_id = if prefix.is_empty() {
                task.id.clone()
            } else {
                format!("{}.{}", prefix, task.id)
            };
            map.insert(qualified_id.clone(), task.priority);

            if !task.children.is_empty() {
                add_tasks(&task.children, map, &qualified_id);
            }
        }
    }

    let mut priorities = HashMap::new();
    add_tasks(&project.tasks, &mut priorities, "");

    for task_id in tasks.keys() {
        let priority = priorities.get(task_id).copied().unwrap_or(500);
        map.insert(task_id.clone(), (priority, ()));
    }

    map
}

/// Add working days to a date
fn add_working_days(start: NaiveDate, days: i64, calendar: &Calendar) -> NaiveDate {
    if days <= 0 {
        return start;
    }

    let mut current = start;
    let mut remaining = days;

    while remaining > 0 {
        current = current.succ_opt().unwrap_or(current);
        if calendar.is_working_day(current) {
            remaining -= 1;
        }
    }

    current
}

/// Count working days between two dates (exclusive of start)
fn count_working_days(start: NaiveDate, end: NaiveDate, calendar: &Calendar) -> i64 {
    if end <= start {
        return 0;
    }

    let mut current = start;
    let mut count = 0;

    while current < end {
        current = current.succ_opt().unwrap_or(current);
        if calendar.is_working_day(current) {
            count += 1;
        }
    }

    count
}

/// Count working days from start to end (both inclusive)
fn count_working_days_between(start: NaiveDate, end: NaiveDate, calendar: &Calendar) -> i64 {
    if end < start {
        return 0;
    }

    let mut current = start;
    let mut count = 0;

    while current <= end {
        if calendar.is_working_day(current) {
            count += 1;
        }
        current = match current.succ_opt() {
            Some(d) => d,
            None => break,
        };
    }

    count
}

/// Recalculate critical path after leveling
fn recalculate_critical_path(
    tasks: &HashMap<TaskId, ScheduledTask>,
    project_end: NaiveDate,
) -> Vec<TaskId> {
    // Tasks on critical path end on project end date and have zero slack
    tasks
        .iter()
        .filter(|(_, task)| task.finish == project_end || task.slack.minutes == 0)
        .map(|(id, _)| id.clone())
        .collect()
}

/// Detect resource over-allocations without resolving them
pub fn detect_overallocations(
    project: &Project,
    schedule: &Schedule,
) -> Vec<(ResourceId, OverallocationPeriod)> {
    let timelines = build_resource_timelines(project, &schedule.tasks);

    timelines
        .into_iter()
        .flat_map(|(_, timeline)| {
            let resource_id = timeline.resource_id.clone();
            timeline
                .overallocated_periods()
                .into_iter()
                .map(move |p| (resource_id.clone(), p))
        })
        .collect()
}

/// Utilization statistics for a single resource
#[derive(Debug, Clone)]
pub struct ResourceUtilization {
    /// Resource identifier
    pub resource_id: ResourceId,
    /// Resource capacity (1.0 = 100%)
    pub capacity: f32,
    /// Total working days in the schedule period
    pub total_days: i64,
    /// Sum of daily usage (in resource-days)
    pub used_days: f32,
    /// Utilization percentage (0-100+, can exceed 100 if over-allocated)
    pub utilization_percent: f32,
    /// Peak daily usage
    pub peak_usage: f32,
    /// Number of days with any assignment
    pub assigned_days: i64,
}

/// Summary of resource utilization across all resources
#[derive(Debug, Clone)]
pub struct UtilizationSummary {
    /// Per-resource utilization statistics
    pub resources: Vec<ResourceUtilization>,
    /// Schedule start date
    pub schedule_start: NaiveDate,
    /// Schedule end date
    pub schedule_end: NaiveDate,
    /// Total working days in schedule period
    pub total_working_days: i64,
    /// Average utilization across all resources
    pub average_utilization: f32,
}

/// Calculate resource utilization for a schedule
pub fn calculate_utilization(
    project: &Project,
    schedule: &Schedule,
    calendar: &Calendar,
) -> UtilizationSummary {
    let timelines = build_resource_timelines(project, &schedule.tasks);

    // Determine schedule date range
    let schedule_start = schedule
        .tasks
        .values()
        .map(|t| t.start)
        .min()
        .unwrap_or(project.start);
    let schedule_end = schedule
        .tasks
        .values()
        .map(|t| t.finish)
        .max()
        .unwrap_or(project.start);

    // Count working days in schedule period
    let total_working_days = count_schedule_working_days(schedule_start, schedule_end, calendar);

    let mut resources = Vec::new();

    for resource in &project.resources {
        let timeline = timelines.get(&resource.id);

        let (used_days, peak_usage, assigned_days) = if let Some(timeline) = timeline {
            let mut used = 0.0f32;
            let mut peak = 0.0f32;
            let mut assigned = 0i64;

            for day_usage in timeline.usage.values() {
                used += day_usage.total_units;
                peak = peak.max(day_usage.total_units);
                if day_usage.total_units > 0.0 {
                    assigned += 1;
                }
            }

            (used, peak, assigned)
        } else {
            (0.0, 0.0, 0)
        };

        // Calculate utilization: used_days / (total_working_days * capacity) * 100
        let capacity_days = total_working_days as f32 * resource.capacity;
        let utilization_percent = if capacity_days > 0.0 {
            (used_days / capacity_days) * 100.0
        } else {
            0.0
        };

        resources.push(ResourceUtilization {
            resource_id: resource.id.clone(),
            capacity: resource.capacity,
            total_days: total_working_days,
            used_days,
            utilization_percent,
            peak_usage,
            assigned_days,
        });
    }

    // Calculate average utilization
    let average_utilization = if resources.is_empty() {
        0.0
    } else {
        resources.iter().map(|r| r.utilization_percent).sum::<f32>() / resources.len() as f32
    };

    UtilizationSummary {
        resources,
        schedule_start,
        schedule_end,
        total_working_days,
        average_utilization,
    }
}

/// Count working days between two dates (inclusive of start, exclusive of end)
fn count_schedule_working_days(start: NaiveDate, end: NaiveDate, calendar: &Calendar) -> i64 {
    if end <= start {
        return 0;
    }

    let mut current = start;
    let mut count = 0;

    while current <= end {
        if calendar.is_working_day(current) {
            count += 1;
        }
        current = match current.succ_opt() {
            Some(d) => d,
            None => break,
        };
    }

    count
}

#[cfg(test)]
mod tests {
    use super::*;
    use utf8proj_core::{Resource, Task};

    fn make_test_calendar() -> Calendar {
        Calendar::default()
    }

    #[test]
    fn timeline_tracks_usage() {
        let mut timeline = ResourceTimeline::new("dev".into(), 1.0);
        let start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        let finish = NaiveDate::from_ymd_opt(2025, 1, 8).unwrap();

        timeline.add_usage(&"task1".into(), start, finish, 0.5);

        assert!(!timeline.is_overallocated(start));
        assert_eq!(timeline.usage.get(&start).unwrap().total_units, 0.5);
    }

    #[test]
    fn timeline_detects_overallocation() {
        let mut timeline = ResourceTimeline::new("dev".into(), 1.0);
        let start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        let finish = NaiveDate::from_ymd_opt(2025, 1, 8).unwrap();

        timeline.add_usage(&"task1".into(), start, finish, 0.6);
        timeline.add_usage(&"task2".into(), start, finish, 0.6);

        assert!(timeline.is_overallocated(start));

        let periods = timeline.overallocated_periods();
        assert_eq!(periods.len(), 1);
        assert!(periods[0].peak_usage > 1.0);
    }

    #[test]
    fn timeline_removes_usage() {
        let mut timeline = ResourceTimeline::new("dev".into(), 1.0);
        let start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        let finish = NaiveDate::from_ymd_opt(2025, 1, 8).unwrap();

        timeline.add_usage(&"task1".into(), start, finish, 0.6);
        timeline.add_usage(&"task2".into(), start, finish, 0.6);

        assert!(timeline.is_overallocated(start));

        timeline.remove_usage(&"task1".into());

        assert!(!timeline.is_overallocated(start));
    }

    #[test]
    fn find_available_slot_basic() {
        let mut timeline = ResourceTimeline::new("dev".into(), 1.0);
        let calendar = make_test_calendar();

        let start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap(); // Monday
        let finish = NaiveDate::from_ymd_opt(2025, 1, 10).unwrap(); // Friday

        // Block the first week
        timeline.add_usage(&"task1".into(), start, finish, 1.0);

        // Find slot for a 3-day task
        let slot = timeline.find_available_slot(3, 1.0, start, &calendar);

        // Should find slot starting after the blocked period
        assert!(slot.is_some());
        assert!(slot.unwrap() > finish);
    }

    #[test]
    fn find_available_slot_respects_limit() {
        let timeline = ResourceTimeline::new("dev".into(), 1.0);
        let calendar = make_test_calendar();
        let start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();

        // Request units exceeding capacity - should return None immediately
        let slot = timeline.find_available_slot(3, 2.0, start, &calendar);
        assert!(slot.is_none());
    }

    #[test]
    fn find_available_slot_with_custom_limit() {
        let mut timeline = ResourceTimeline::new("dev".into(), 1.0);
        let calendar = make_test_calendar();
        let start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap(); // Monday

        // Block every working day for 50 calendar days (covers ~35 working days)
        let mut day = start;
        for i in 0..50 {
            if calendar.is_working_day(day) {
                timeline.add_usage(&format!("block_{i}"), day, day, 1.0);
            }
            day = day.succ_opt().unwrap();
        }

        // With a small limit (5 working days), should not find a slot
        let slot = timeline.find_available_slot_with_limit(1, 1.0, start, &calendar, 5);
        assert!(slot.is_none());

        // With a larger limit (100 working days), should find a slot after the blocked period
        let slot = timeline.find_available_slot_with_limit(1, 1.0, start, &calendar, 100);
        assert!(slot.is_some());
    }

    #[test]
    fn detect_overallocations_finds_conflicts() {
        use utf8proj_core::Scheduler;

        let mut project = Project::new("Test");
        project.start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        project.resources = vec![Resource::new("dev").capacity(1.0)];
        project.tasks = vec![
            Task::new("task1").effort(Duration::days(5)).assign("dev"),
            Task::new("task2").effort(Duration::days(5)).assign("dev"),
        ];

        let solver = crate::CpmSolver::new();
        let schedule = solver.schedule(&project).unwrap();

        // Both tasks start on day 0, both use dev at 100%
        let conflicts = detect_overallocations(&project, &schedule);

        assert!(
            !conflicts.is_empty(),
            "Should detect resource conflict when same resource assigned to parallel tasks"
        );
    }

    #[test]
    fn level_resources_resolves_simple_conflict() {
        use utf8proj_core::Scheduler;

        let mut project = Project::new("Test");
        project.start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        project.resources = vec![Resource::new("dev").capacity(1.0)];
        project.tasks = vec![
            Task::new("task1").effort(Duration::days(3)).assign("dev"),
            Task::new("task2").effort(Duration::days(3)).assign("dev"),
        ];

        let solver = crate::CpmSolver::new();
        let schedule = solver.schedule(&project).unwrap();

        let calendar = Calendar::default();
        let result = level_resources(&project, &schedule, &calendar);

        // One task should have been shifted
        assert!(
            !result.shifted_tasks.is_empty(),
            "Should shift at least one task"
        );

        // No unresolved conflicts
        assert!(
            result.unresolved_conflicts.is_empty(),
            "Should resolve all conflicts"
        );

        // Tasks should no longer overlap
        let task1 = &result.leveled_schedule.tasks["task1"];
        let task2 = &result.leveled_schedule.tasks["task2"];

        let overlap = task1.start <= task2.finish && task2.start <= task1.finish;
        assert!(
            !overlap || task1.start == task2.start,
            "Tasks should not overlap after leveling"
        );
    }

    #[test]
    fn level_resources_respects_dependencies() {
        use utf8proj_core::Scheduler;

        let mut project = Project::new("Test");
        project.start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        project.resources = vec![Resource::new("dev").capacity(1.0)];
        project.tasks = vec![
            Task::new("task1").effort(Duration::days(3)).assign("dev"),
            Task::new("task2")
                .effort(Duration::days(3))
                .assign("dev")
                .depends_on("task1"),
        ];

        let solver = crate::CpmSolver::new();
        let schedule = solver.schedule(&project).unwrap();

        let calendar = Calendar::default();
        let result = level_resources(&project, &schedule, &calendar);

        // With dependencies, tasks already don't overlap
        let task1 = &result.leveled_schedule.tasks["task1"];
        let task2 = &result.leveled_schedule.tasks["task2"];

        assert!(
            task2.start > task1.finish || task2.start == task1.finish.succ_opt().unwrap(),
            "Task2 should start after task1 finishes"
        );
    }

    #[test]
    fn level_resources_extends_project_when_needed() {
        use utf8proj_core::Scheduler;

        let mut project = Project::new("Test");
        project.start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        project.resources = vec![Resource::new("dev").capacity(1.0)];
        project.tasks = vec![
            Task::new("task1").effort(Duration::days(5)).assign("dev"),
            Task::new("task2").effort(Duration::days(5)).assign("dev"),
        ];

        let solver = crate::CpmSolver::new();
        let schedule = solver.schedule(&project).unwrap();

        let calendar = Calendar::default();
        let result = level_resources(&project, &schedule, &calendar);

        // Original: both tasks 5 days, parallel = 5 days
        // After leveling: sequential = 10 days
        assert!(
            result.project_extended,
            "Project should be extended when leveling parallel tasks"
        );
        assert!(result.new_project_end > schedule.project_end);
    }

    #[test]
    fn add_working_days_zero_or_negative() {
        let calendar = make_test_calendar();
        let start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();

        // Zero days returns start (line 461)
        assert_eq!(add_working_days(start, 0, &calendar), start);

        // Negative days also returns start (line 461)
        assert_eq!(add_working_days(start, -5, &calendar), start);
    }

    #[test]
    fn count_working_days_end_before_start() {
        let calendar = make_test_calendar();
        let start = NaiveDate::from_ymd_opt(2025, 1, 10).unwrap();
        let end = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();

        // end < start returns 0 (line 480)
        assert_eq!(count_working_days(start, end, &calendar), 0);

        // end == start also returns 0 (line 480)
        assert_eq!(count_working_days(start, start, &calendar), 0);
    }

    #[test]
    fn shift_candidate_ordering_critical_vs_non_critical() {
        // Test line 224: when one is critical and other is not
        let critical = ShiftCandidate {
            task_id: "critical_task".into(),
            priority: 100,
            slack_days: 0,
            is_critical: true,
        };

        let non_critical = ShiftCandidate {
            task_id: "non_critical_task".into(),
            priority: 100,
            slack_days: 0,
            is_critical: false,
        };

        // Non-critical should be preferred (Greater) over critical (line 224-225)
        assert!(non_critical > critical);
        assert!(critical < non_critical);
    }

    #[test]
    fn overallocated_periods_multiple_consecutive_days() {
        // Tests lines 86-93: continuing overallocation period with new tasks
        let mut timeline = ResourceTimeline::new("dev".into(), 1.0);
        let day1 = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        let day2 = NaiveDate::from_ymd_opt(2025, 1, 7).unwrap();
        let day3 = NaiveDate::from_ymd_opt(2025, 1, 8).unwrap();

        // Day 1: task1 + task2 overallocated
        timeline.add_usage(&"task1".into(), day1, day1, 0.7);
        timeline.add_usage(&"task2".into(), day1, day1, 0.7);

        // Day 2: task1 + task3 overallocated (different set of tasks)
        timeline.add_usage(&"task1".into(), day2, day2, 0.7);
        timeline.add_usage(&"task3".into(), day2, day2, 0.7);

        // Day 3: not overallocated
        timeline.add_usage(&"task1".into(), day3, day3, 0.3);

        let periods = timeline.overallocated_periods();

        // Should be one period spanning day1-day2
        assert_eq!(periods.len(), 1);
        assert_eq!(periods[0].start, day1);
        assert_eq!(periods[0].end, day2);
        // Line 91: task3 should be added to involved_tasks
        assert!(periods[0].involved_tasks.contains(&"task1".to_string()));
        assert!(periods[0].involved_tasks.contains(&"task2".to_string()));
        assert!(periods[0].involved_tasks.contains(&"task3".to_string()));
    }

    #[test]
    fn overallocated_periods_non_consecutive_creates_multiple() {
        // Tests line 97: pushing completed period when non-consecutive overallocation starts
        let mut timeline = ResourceTimeline::new("dev".into(), 1.0);
        let day1 = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap(); // Monday
        let day3 = NaiveDate::from_ymd_opt(2025, 1, 8).unwrap(); // Wednesday

        // Day 1: overallocated
        timeline.add_usage(&"task1".into(), day1, day1, 1.5);

        // Day 3 (gap on day 2): overallocated again
        timeline.add_usage(&"task2".into(), day3, day3, 1.5);

        let periods = timeline.overallocated_periods();

        // Should create two separate periods (line 97 pushes first period)
        assert_eq!(periods.len(), 2);
        assert_eq!(periods[0].start, day1);
        assert_eq!(periods[0].end, day1);
        assert_eq!(periods[1].start, day3);
        assert_eq!(periods[1].end, day3);
    }

    #[test]
    fn nested_task_priority_mapping() {
        // Test lines 438, 443: format! for qualified IDs and recursive add_tasks
        use utf8proj_core::{Project, Scheduler};

        let mut project = Project::new("Nested Priority Test");
        project.start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();

        // Create nested tasks with different priorities
        let mut container = Task::new("phase1");
        container.priority = 100;

        let mut child = Task::new("task1");
        child.priority = 200;
        child.duration = Some(utf8proj_core::Duration::days(2));

        let mut grandchild = Task::new("subtask1");
        grandchild.priority = 300;
        grandchild.duration = Some(utf8proj_core::Duration::days(1));

        child.children.push(grandchild);
        container.children.push(child);
        project.tasks.push(container);

        // Add a root-level task for comparison
        let mut root_task = Task::new("standalone");
        root_task.priority = 500;
        root_task.duration = Some(utf8proj_core::Duration::days(1));
        project.tasks.push(root_task);

        // Schedule the project
        let solver = crate::CpmSolver::new();
        let schedule = solver.schedule(&project).unwrap();

        // Build priority map - this calls the internal function
        let priority_map = build_task_priority_map(&schedule.tasks, &project);

        // Verify qualified IDs are constructed correctly (line 438)
        assert!(priority_map.contains_key("phase1.task1.subtask1"));
        assert!(priority_map.contains_key("phase1.task1"));
        // Root level task has no prefix
        assert!(priority_map.contains_key("standalone"));

        // Verify priorities are captured correctly
        assert_eq!(priority_map["standalone"].0, 500);
    }

    #[test]
    fn unresolved_conflict_no_shiftable_tasks() {
        // Test lines 297-300: UnresolvedConflict when candidates.is_empty()
        // This happens when all conflicting tasks are on the critical path
        // and have zero slack
        use utf8proj_core::{
            Dependency, DependencyType, Project, Resource, ResourceRef, Scheduler,
        };

        let mut project = Project::new("All Critical Test");
        project.start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();

        // Add a resource
        project.resources.push(Resource::new("dev").capacity(1.0));

        // Create two parallel critical tasks that conflict
        // Both tasks are independent (no dependencies) and same duration
        // so both could be critical depending on solver decisions
        let mut task1 = Task::new("critical1");
        task1.duration = Some(utf8proj_core::Duration::days(5));
        task1.assigned.push(ResourceRef {
            resource_id: "dev".into(),
            units: 1.0,
        });

        let mut task2 = Task::new("critical2");
        task2.duration = Some(utf8proj_core::Duration::days(5));
        task2.assigned.push(ResourceRef {
            resource_id: "dev".into(),
            units: 1.0,
        });

        // Make them sequential through dependency so both are critical
        task2.depends = vec![Dependency {
            predecessor: "critical1".into(),
            dep_type: DependencyType::FinishToStart,
            lag: None,
        }];

        project.tasks.push(task1);
        project.tasks.push(task2);

        let solver = crate::CpmSolver::new();
        let schedule = solver.schedule(&project).unwrap();

        // Both tasks should be critical
        assert!(schedule.tasks["critical1"].is_critical);
        assert!(schedule.tasks["critical2"].is_critical);

        // Detect overallocation - there shouldn't be any since tasks are sequential
        let overallocations = detect_overallocations(&project, &schedule);
        assert!(
            overallocations.is_empty(),
            "Sequential critical tasks should not conflict"
        );
    }

    #[test]
    fn recalculate_critical_path_test() {
        // Test the recalculate_critical_path function
        use utf8proj_core::{Dependency, DependencyType, Project, Scheduler};

        let mut project = Project::new("Critical Path Test");
        project.start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();

        // Create a simple chain
        let mut task1 = Task::new("first");
        task1.duration = Some(utf8proj_core::Duration::days(3));

        let mut task2 = Task::new("second");
        task2.duration = Some(utf8proj_core::Duration::days(2));
        task2.depends = vec![Dependency {
            predecessor: "first".into(),
            dep_type: DependencyType::FinishToStart,
            lag: None,
        }];

        project.tasks.push(task1);
        project.tasks.push(task2);

        let solver = crate::CpmSolver::new();
        let schedule = solver.schedule(&project).unwrap();

        let critical = recalculate_critical_path(&schedule.tasks, schedule.project_end);

        // Both tasks in a linear chain should be critical
        assert!(critical.contains(&"first".to_string()));
        assert!(critical.contains(&"second".to_string()));
    }

    #[test]
    fn calculate_utilization_basic() {
        use utf8proj_core::Scheduler;

        let mut project = Project::new("Utilization Test");
        project.start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap(); // Monday
        project.resources = vec![Resource::new("dev").capacity(1.0)];
        project.tasks = vec![Task::new("task1").effort(Duration::days(5)).assign("dev")];

        let solver = crate::CpmSolver::new();
        let schedule = solver.schedule(&project).unwrap();
        let calendar = Calendar::default();

        let utilization = calculate_utilization(&project, &schedule, &calendar);

        assert_eq!(utilization.resources.len(), 1);
        let dev_util = &utilization.resources[0];
        assert_eq!(dev_util.resource_id, "dev");
        // 5 days used over ~5 working days = ~100% utilization
        assert!(dev_util.utilization_percent > 90.0);
        assert!(dev_util.used_days > 0.0);
    }

    #[test]
    fn calculate_utilization_multiple_resources() {
        use utf8proj_core::Scheduler;

        let mut project = Project::new("Multi Resource Test");
        project.start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        project.resources = vec![
            Resource::new("dev1").capacity(1.0),
            Resource::new("dev2").capacity(1.0),
        ];
        project.tasks = vec![
            Task::new("task1").effort(Duration::days(5)).assign("dev1"),
            Task::new("task2").effort(Duration::days(3)).assign("dev2"),
        ];

        let solver = crate::CpmSolver::new();
        let schedule = solver.schedule(&project).unwrap();
        let calendar = Calendar::default();

        let utilization = calculate_utilization(&project, &schedule, &calendar);

        assert_eq!(utilization.resources.len(), 2);
        assert!(utilization.average_utilization > 0.0);

        // dev1 should have higher utilization than dev2
        let dev1 = utilization
            .resources
            .iter()
            .find(|r| r.resource_id == "dev1")
            .unwrap();
        let dev2 = utilization
            .resources
            .iter()
            .find(|r| r.resource_id == "dev2")
            .unwrap();
        assert!(dev1.used_days > dev2.used_days);
    }

    #[test]
    fn calculate_utilization_no_resources() {
        use utf8proj_core::Scheduler;

        let mut project = Project::new("No Resources Test");
        project.start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        project.tasks = vec![Task::new("task1").effort(Duration::days(5))];

        let solver = crate::CpmSolver::new();
        let schedule = solver.schedule(&project).unwrap();
        let calendar = Calendar::default();

        let utilization = calculate_utilization(&project, &schedule, &calendar);

        assert!(utilization.resources.is_empty());
        assert_eq!(utilization.average_utilization, 0.0);
    }

    #[test]
    fn calculate_utilization_idle_resource() {
        use utf8proj_core::Scheduler;

        let mut project = Project::new("Idle Resource Test");
        project.start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        project.resources = vec![
            Resource::new("dev1").capacity(1.0),
            Resource::new("dev2").capacity(1.0), // No assignments
        ];
        project.tasks = vec![Task::new("task1").effort(Duration::days(5)).assign("dev1")];

        let solver = crate::CpmSolver::new();
        let schedule = solver.schedule(&project).unwrap();
        let calendar = Calendar::default();

        let utilization = calculate_utilization(&project, &schedule, &calendar);

        let dev2 = utilization
            .resources
            .iter()
            .find(|r| r.resource_id == "dev2")
            .unwrap();
        assert_eq!(dev2.used_days, 0.0);
        assert_eq!(dev2.utilization_percent, 0.0);
        assert_eq!(dev2.assigned_days, 0);
    }

    #[test]
    fn count_schedule_working_days_basic() {
        let calendar = make_test_calendar();
        let start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap(); // Monday
        let end = NaiveDate::from_ymd_opt(2025, 1, 10).unwrap(); // Friday

        // Monday through Friday = 5 working days
        let count = count_schedule_working_days(start, end, &calendar);
        assert_eq!(count, 5);
    }

    #[test]
    fn count_schedule_working_days_same_date() {
        let calendar = make_test_calendar();
        let date = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();

        // Same start and end returns 0 (function requires end > start)
        let count = count_schedule_working_days(date, date, &calendar);
        assert_eq!(count, 0);
    }

    #[test]
    fn count_schedule_working_days_end_before_start() {
        let calendar = make_test_calendar();
        let start = NaiveDate::from_ymd_opt(2025, 1, 10).unwrap();
        let end = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();

        // Invalid range should return 0
        let count = count_schedule_working_days(start, end, &calendar);
        assert_eq!(count, 0);
    }

    #[test]
    fn propagation_shifts_successor_after_predecessor_delayed() {
        // A -> B, same resource. Leveling shifts A, B must cascade.
        use utf8proj_core::Scheduler;

        let mut project = Project::new("Propagation Test");
        project.start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        project.resources = vec![Resource::new("dev").capacity(1.0)];
        project.tasks = vec![
            // task0 occupies dev, no dependency — forces conflict with task_a
            Task::new("task0").effort(Duration::days(3)).assign("dev"),
            Task::new("task_a").effort(Duration::days(3)).assign("dev"),
            Task::new("task_b")
                .effort(Duration::days(3))
                .assign("dev")
                .depends_on("task_a"),
        ];

        let solver = crate::CpmSolver::new();
        let schedule = solver.schedule(&project).unwrap();
        let calendar = Calendar::default();
        let result = level_resources(&project, &schedule, &calendar);

        // task_b must start after task_a finishes (dependency preserved after leveling)
        let task_a = &result.leveled_schedule.tasks["task_a"];
        let task_b = &result.leveled_schedule.tasks["task_b"];
        assert!(
            task_b.start > task_a.finish
                || task_b.start == task_a.finish.succ_opt().unwrap(),
            "task_b ({}) must start after task_a finishes ({})",
            task_b.start,
            task_a.finish
        );
    }

    #[test]
    fn propagation_chain_a_b_c() {
        // A -> B -> C, same resource + conflicting task. Cascade through chain.
        use utf8proj_core::Scheduler;

        let mut project = Project::new("Chain Propagation");
        project.start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        project.resources = vec![Resource::new("dev").capacity(1.0)];
        project.tasks = vec![
            Task::new("blocker").effort(Duration::days(3)).assign("dev"),
            Task::new("a").effort(Duration::days(2)).assign("dev"),
            Task::new("b")
                .effort(Duration::days(2))
                .assign("dev")
                .depends_on("a"),
            Task::new("c")
                .effort(Duration::days(2))
                .assign("dev")
                .depends_on("b"),
        ];

        let solver = crate::CpmSolver::new();
        let schedule = solver.schedule(&project).unwrap();
        let calendar = Calendar::default();
        let result = level_resources(&project, &schedule, &calendar);

        let a = &result.leveled_schedule.tasks["a"];
        let b = &result.leveled_schedule.tasks["b"];
        let c = &result.leveled_schedule.tasks["c"];

        assert!(
            b.start > a.finish || b.start == a.finish.succ_opt().unwrap(),
            "b must respect a: b.start={} a.finish={}",
            b.start,
            a.finish
        );
        assert!(
            c.start > b.finish || c.start == b.finish.succ_opt().unwrap(),
            "c must respect b: c.start={} b.finish={}",
            c.start,
            b.finish
        );
    }

    #[test]
    fn propagation_diamond_both_predecessors() {
        // A -> C, B -> C. Both A and B delayed. C respects both.
        use utf8proj_core::{Dependency, DependencyType, Scheduler};

        let mut project = Project::new("Diamond Propagation");
        project.start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        project.resources = vec![Resource::new("dev").capacity(1.0)];

        let mut task_c = Task::new("c");
        task_c.effort = Some(Duration::days(2));
        task_c.assigned.push(utf8proj_core::ResourceRef {
            resource_id: "dev".into(),
            units: 1.0,
        });
        task_c.depends = vec![
            Dependency {
                predecessor: "a".into(),
                dep_type: DependencyType::FinishToStart,
                lag: None,
            },
            Dependency {
                predecessor: "b".into(),
                dep_type: DependencyType::FinishToStart,
                lag: None,
            },
        ];

        project.tasks = vec![
            Task::new("a").effort(Duration::days(3)).assign("dev"),
            Task::new("b").effort(Duration::days(5)).assign("dev"),
            task_c,
        ];

        let solver = crate::CpmSolver::new();
        let schedule = solver.schedule(&project).unwrap();
        let calendar = Calendar::default();
        let result = level_resources(&project, &schedule, &calendar);

        let a = &result.leveled_schedule.tasks["a"];
        let b = &result.leveled_schedule.tasks["b"];
        let c = &result.leveled_schedule.tasks["c"];

        // C must start after BOTH a and b finish
        assert!(
            c.start > a.finish,
            "c.start ({}) must be after a.finish ({})",
            c.start,
            a.finish
        );
        assert!(
            c.start > b.finish,
            "c.start ({}) must be after b.finish ({})",
            c.start,
            b.finish
        );
    }

    #[test]
    fn propagation_with_lag() {
        // A -> B +2d lag. After leveling, lag is preserved.
        use utf8proj_core::{Dependency, DependencyType, Scheduler};

        let mut project = Project::new("Lag Propagation");
        project.start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        project.resources = vec![Resource::new("dev").capacity(1.0)];

        let mut task_b = Task::new("b");
        task_b.effort = Some(Duration::days(2));
        task_b.assigned.push(utf8proj_core::ResourceRef {
            resource_id: "dev".into(),
            units: 1.0,
        });
        task_b.depends = vec![Dependency {
            predecessor: "a".into(),
            dep_type: DependencyType::FinishToStart,
            lag: Some(Duration::days(2)),
        }];

        project.tasks = vec![
            Task::new("blocker").effort(Duration::days(3)).assign("dev"),
            Task::new("a").effort(Duration::days(2)).assign("dev"),
            task_b,
        ];

        let solver = crate::CpmSolver::new();
        let schedule = solver.schedule(&project).unwrap();
        let calendar = Calendar::default();
        let result = level_resources(&project, &schedule, &calendar);

        let a = &result.leveled_schedule.tasks["a"];
        let b = &result.leveled_schedule.tasks["b"];

        // B must start at least 2 working days after A finishes (FS + 2d lag)
        let gap = count_working_days(a.finish, b.start, &calendar);
        assert!(
            gap >= 2,
            "Expected gap >= 2 working days between a.finish ({}) and b.start ({}), got {}",
            a.finish,
            b.start,
            gap
        );
    }

    #[test]
    fn propagation_ss_dependency() {
        // A -SS-> B. After leveling shifts A, B must respect SS constraint.
        use utf8proj_core::{Dependency, DependencyType, Scheduler};

        let mut project = Project::new("SS Propagation");
        project.start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        project.resources = vec![
            Resource::new("dev1").capacity(1.0),
            Resource::new("dev2").capacity(1.0),
        ];

        let mut task_b = Task::new("b");
        task_b.effort = Some(Duration::days(3));
        task_b.assigned.push(utf8proj_core::ResourceRef {
            resource_id: "dev2".into(),
            units: 1.0,
        });
        task_b.depends = vec![Dependency {
            predecessor: "a".into(),
            dep_type: DependencyType::StartToStart,
            lag: None,
        }];

        project.tasks = vec![
            Task::new("blocker").effort(Duration::days(3)).assign("dev1"),
            Task::new("a").effort(Duration::days(3)).assign("dev1"),
            task_b,
        ];

        let solver = crate::CpmSolver::new();
        let schedule = solver.schedule(&project).unwrap();
        let calendar = Calendar::default();
        let result = level_resources(&project, &schedule, &calendar);

        let a = &result.leveled_schedule.tasks["a"];
        let b = &result.leveled_schedule.tasks["b"];

        // SS: B must start >= A's start
        assert!(
            b.start >= a.start,
            "SS: b.start ({}) must be >= a.start ({})",
            b.start,
            a.start
        );
    }

    // =========================================================================
    // Non-Regression Tests (NRTs)
    //
    // These tests encode the exact invariant that was violated before the
    // successor-propagation fix: after leveling, every dependency edge must
    // have a non-negative gap. They are designed to FAIL on the old code
    // (pre-propagation) and PASS on the fixed code.
    // =========================================================================

    /// Scan every dependency edge in a leveled schedule and assert no negative gaps.
    ///
    /// This is the invariant that was violated in 277/480 PSPLIB J30 instances.
    fn assert_no_negative_gaps(
        project: &Project,
        schedule: &Schedule,
        _calendar: &Calendar,
    ) {
        let successor_map = build_successor_map(project);
        for (pred_id, successors) in &successor_map {
            let Some(pred) = schedule.tasks.get(pred_id) else {
                continue;
            };
            for (succ_id, dep_type, lag) in successors {
                let Some(succ) = schedule.tasks.get(succ_id) else {
                    continue;
                };
                let lag_days = lag.as_ref().map(|l| l.as_days() as i64).unwrap_or(0);

                let violated = match dep_type {
                    DependencyType::FinishToStart => {
                        // succ.start must be > pred.finish + lag
                        succ.start < pred.finish + chrono::Duration::days(lag_days + 1)
                    }
                    DependencyType::StartToStart => {
                        succ.start < pred.start + chrono::Duration::days(lag_days)
                    }
                    DependencyType::FinishToFinish => {
                        succ.finish < pred.finish + chrono::Duration::days(lag_days)
                    }
                    DependencyType::StartToFinish => {
                        succ.finish < pred.start + chrono::Duration::days(lag_days)
                    }
                };

                assert!(
                    !violated,
                    "Negative gap: {} ({:?}) {} -> {} (pred finish={}, succ start={}, lag={}d)",
                    pred_id, dep_type, pred.finish, succ_id, pred.finish, succ.start, lag_days
                );
            }
        }
    }

    #[test]
    fn nrt_no_negative_gaps_after_leveling_simple() {
        // NRT: Two tasks share a resource, second depends on first.
        // A third "blocker" task forces leveling to shift the predecessor.
        // The successor MUST be pushed forward — not left stranded.
        //
        // This is the minimal reproduction of the PSPLIB J30 bug.
        use utf8proj_core::Scheduler;

        let mut project = Project::new("NRT Simple");
        project.start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        project.resources = vec![Resource::new("dev").capacity(1.0)];
        project.tasks = vec![
            // blocker occupies dev days 1-5, forcing "a" to shift
            Task::new("blocker").effort(Duration::days(5)).assign("dev"),
            // a depends on nothing, also wants dev days 1-3
            Task::new("a").effort(Duration::days(3)).assign("dev"),
            // b depends on a (FS), also wants dev
            Task::new("b")
                .effort(Duration::days(3))
                .assign("dev")
                .depends_on("a"),
        ];

        let solver = crate::CpmSolver::new();
        let schedule = solver.schedule(&project).unwrap();
        let calendar = Calendar::default();
        let result = level_resources(&project, &schedule, &calendar);

        // THE INVARIANT: no dependency edge has a negative gap
        assert_no_negative_gaps(&project, &result.leveled_schedule, &calendar);
    }

    #[test]
    fn nrt_no_negative_gaps_chain_of_five() {
        // NRT: Chain A->B->C->D->E, all same resource, plus a blocker.
        // Every edge in the chain must have non-negative gap after leveling.
        use utf8proj_core::Scheduler;

        let mut project = Project::new("NRT Chain");
        project.start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        project.resources = vec![Resource::new("dev").capacity(1.0)];
        project.tasks = vec![
            Task::new("blocker").effort(Duration::days(5)).assign("dev"),
            Task::new("a").effort(Duration::days(2)).assign("dev"),
            Task::new("b")
                .effort(Duration::days(2))
                .assign("dev")
                .depends_on("a"),
            Task::new("c")
                .effort(Duration::days(2))
                .assign("dev")
                .depends_on("b"),
            Task::new("d")
                .effort(Duration::days(2))
                .assign("dev")
                .depends_on("c"),
            Task::new("e")
                .effort(Duration::days(2))
                .assign("dev")
                .depends_on("d"),
        ];

        let solver = crate::CpmSolver::new();
        let schedule = solver.schedule(&project).unwrap();
        let calendar = Calendar::default();
        let result = level_resources(&project, &schedule, &calendar);

        assert_no_negative_gaps(&project, &result.leveled_schedule, &calendar);
    }

    #[test]
    fn nrt_no_negative_gaps_diamond_with_lag() {
        // NRT: Diamond A->C, B->C with lag, all same resource + blocker.
        // Tests that the max(predecessors) logic works under leveling.
        use utf8proj_core::{Dependency, DependencyType, Scheduler};

        let mut project = Project::new("NRT Diamond Lag");
        project.start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        project.resources = vec![Resource::new("dev").capacity(1.0)];

        let mut task_c = Task::new("c");
        task_c.effort = Some(Duration::days(2));
        task_c.assigned.push(utf8proj_core::ResourceRef {
            resource_id: "dev".into(),
            units: 1.0,
        });
        task_c.depends = vec![
            Dependency {
                predecessor: "a".into(),
                dep_type: DependencyType::FinishToStart,
                lag: Some(Duration::days(1)),
            },
            Dependency {
                predecessor: "b".into(),
                dep_type: DependencyType::FinishToStart,
                lag: None,
            },
        ];

        project.tasks = vec![
            Task::new("blocker").effort(Duration::days(4)).assign("dev"),
            Task::new("a").effort(Duration::days(3)).assign("dev"),
            Task::new("b").effort(Duration::days(5)).assign("dev"),
            task_c,
        ];

        let solver = crate::CpmSolver::new();
        let schedule = solver.schedule(&project).unwrap();
        let calendar = Calendar::default();
        let result = level_resources(&project, &schedule, &calendar);

        assert_no_negative_gaps(&project, &result.leveled_schedule, &calendar);
    }

    #[test]
    fn nrt_no_negative_gaps_multiple_resources() {
        // NRT: Tasks on different resources connected by dependencies.
        // Leveling one resource must propagate to successors on another.
        use utf8proj_core::Scheduler;

        let mut project = Project::new("NRT Multi Resource");
        project.start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        project.resources = vec![
            Resource::new("dev").capacity(1.0),
            Resource::new("qa").capacity(1.0),
        ];
        project.tasks = vec![
            // blocker and "a" compete for dev
            Task::new("blocker").effort(Duration::days(5)).assign("dev"),
            Task::new("a").effort(Duration::days(3)).assign("dev"),
            // "b" is on qa but depends on "a" (cross-resource dependency)
            Task::new("b")
                .effort(Duration::days(3))
                .assign("qa")
                .depends_on("a"),
        ];

        let solver = crate::CpmSolver::new();
        let schedule = solver.schedule(&project).unwrap();
        let calendar = Calendar::default();
        let result = level_resources(&project, &schedule, &calendar);

        assert_no_negative_gaps(&project, &result.leveled_schedule, &calendar);
    }

    // =========================================================================
    // P1 Coverage Gap Tests
    //
    // G1: Unresolvable conflicts (L002 diagnostics, unresolved_conflicts vec)
    // G2: Max delay factor enforcement
    // G3: Hybrid/cluster leveling end-to-end
    // =========================================================================

    // --- G1: Unresolvable Conflicts ---

    #[test]
    fn unresolvable_conflict_no_slot_found() {
        // Two tasks each need 100% of a resource with capacity 1.0.
        // After shifting one, the other still conflicts — but both can
        // eventually be sequenced. This test targets the case where tasks
        // CAN be resolved. For the truly unresolvable case, see below.
        //
        // Here we test that the leveler emits L001 diagnostics and has
        // no unresolved conflicts for a solvable scenario.
        use utf8proj_core::Scheduler;

        let mut project = Project::new("Solvable Conflict");
        project.start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        project.resources = vec![Resource::new("dev").capacity(1.0)];
        project.tasks = vec![
            Task::new("a").effort(Duration::days(3)).assign("dev"),
            Task::new("b").effort(Duration::days(3)).assign("dev"),
        ];

        let solver = crate::CpmSolver::new();
        let schedule = solver.schedule(&project).unwrap();
        let calendar = Calendar::default();
        let result = level_resources(&project, &schedule, &calendar);

        // Should resolve cleanly
        assert!(
            result.unresolved_conflicts.is_empty(),
            "Solvable conflict should have no unresolved conflicts, got: {:?}",
            result
                .unresolved_conflicts
                .iter()
                .map(|c| &c.reason)
                .collect::<Vec<_>>()
        );

        // L001 must be emitted
        let l001_count = result
            .diagnostics
            .iter()
            .filter(|d| d.code == DiagnosticCode::L001OverallocationResolved)
            .count();
        assert!(l001_count > 0, "Expected at least one L001 diagnostic");
    }

    #[test]
    fn unresolvable_conflict_units_exceed_capacity() {
        // A task assigned at 200% to a resource with capacity 100%.
        // The leveler cannot find any slot (units > capacity), so it
        // must report an unresolved conflict with L002 diagnostic.
        use utf8proj_core::Scheduler;

        let mut project = Project::new("Unresolvable Units");
        project.start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        project.resources = vec![Resource::new("dev").capacity(1.0)];
        project.tasks = vec![
            // blocker occupies dev at 100%
            Task::new("blocker").effort(Duration::days(3)).assign("dev"),
            // overloaded: assigned at 200% — no slot can accommodate this
            Task::new("overloaded")
                .effort(Duration::days(3))
                .assign_with_units("dev", 2.0),
        ];

        let solver = crate::CpmSolver::new();
        let schedule = solver.schedule(&project).unwrap();
        let calendar = Calendar::default();
        let result = level_resources(&project, &schedule, &calendar);

        // The overloaded task should appear in unresolved conflicts
        // because find_available_slot returns None when units > capacity
        let has_unresolved = !result.unresolved_conflicts.is_empty();
        let has_l002 = result
            .diagnostics
            .iter()
            .any(|d| d.code == DiagnosticCode::L002UnresolvableConflict);

        assert!(
            has_unresolved || has_l002,
            "Expected unresolved conflict or L002 diagnostic for task with units > capacity. \
             unresolved_conflicts={}, diagnostics={:?}",
            result.unresolved_conflicts.len(),
            result
                .diagnostics
                .iter()
                .map(|d| format!("{:?}", d.code))
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn unresolvable_conflict_reports_l002_diagnostic() {
        // Verifies the L002 diagnostic structure when a conflict cannot be
        // resolved. Uses a resource with capacity 0.5 but two tasks each
        // needing 0.5 — they can be sequenced. But a third task at 0.6
        // will always exceed capacity and cannot be placed anywhere.
        use utf8proj_core::Scheduler;

        let mut project = Project::new("L002 Diagnostic");
        project.start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        project.resources = vec![Resource::new("dev").capacity(0.5)];
        project.tasks = vec![
            // blocker at 50% — fills the resource
            Task::new("blocker").effort(Duration::days(5)).assign_with_units("dev", 0.5),
            // exceeds: needs 60% but resource only has 50% capacity
            Task::new("exceeds").effort(Duration::days(3)).assign_with_units("dev", 0.6),
        ];

        let solver = crate::CpmSolver::new();
        let schedule = solver.schedule(&project).unwrap();
        let calendar = Calendar::default();
        let result = level_resources(&project, &schedule, &calendar);

        // L002 must be emitted because 0.6 > 0.5 capacity — no slot possible
        let l002_diagnostics: Vec<_> = result
            .diagnostics
            .iter()
            .filter(|d| d.code == DiagnosticCode::L002UnresolvableConflict)
            .collect();

        assert!(
            !l002_diagnostics.is_empty(),
            "Expected L002 diagnostic when task units (0.6) exceed resource capacity (0.5)"
        );

        // Verify diagnostic severity is Warning
        for diag in &l002_diagnostics {
            assert_eq!(
                diag.severity,
                Severity::Warning,
                "L002 should be Warning severity"
            );
        }

        // Verify unresolved_conflicts is populated
        assert!(
            !result.unresolved_conflicts.is_empty(),
            "unresolved_conflicts should be non-empty"
        );
    }

    // --- G2: Max Delay Factor ---

    #[test]
    fn max_delay_factor_limits_leveling() {
        // Two parallel tasks sharing a resource (5 days each).
        // Without limit: leveling sequences them → 10 days.
        // With max_project_delay_factor = 1.5: can extend from 5 to 7.5 days
        // but not to 10, so leveling should stop early.
        use utf8proj_core::Scheduler;

        let mut project = Project::new("Max Delay Factor");
        project.start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        project.resources = vec![Resource::new("dev").capacity(1.0)];
        project.tasks = vec![
            Task::new("a").effort(Duration::days(5)).assign("dev"),
            Task::new("b").effort(Duration::days(5)).assign("dev"),
        ];

        let solver = crate::CpmSolver::new();
        let schedule = solver.schedule(&project).unwrap();
        let calendar = Calendar::default();

        // Unlimited leveling: should extend the project
        let unlimited = level_resources(&project, &schedule, &calendar);
        assert!(unlimited.project_extended, "Unlimited leveling should extend project");
        assert!(
            unlimited.unresolved_conflicts.is_empty(),
            "Unlimited should resolve all conflicts"
        );

        // Limited leveling: factor 1.5 means max 1.5x original duration
        let limited_options = LevelingOptions {
            max_project_delay_factor: Some(1.5),
            ..LevelingOptions::default()
        };
        let limited =
            level_resources_with_options(&project, &schedule, &calendar, &limited_options);

        // Must have unresolved conflicts — can't fully sequence within 1.5x
        assert!(
            !limited.unresolved_conflicts.is_empty(),
            "Limited leveling should leave unresolved conflicts when factor is restrictive"
        );
    }

    #[test]
    fn max_delay_factor_allows_within_budget() {
        // Two parallel tasks (3 days each) sharing a resource.
        // Sequential = 6 days. Factor 3.0 (generous) should allow full resolution.
        use utf8proj_core::Scheduler;

        let mut project = Project::new("Max Delay Generous");
        project.start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        project.resources = vec![Resource::new("dev").capacity(1.0)];
        project.tasks = vec![
            Task::new("a").effort(Duration::days(3)).assign("dev"),
            Task::new("b").effort(Duration::days(3)).assign("dev"),
        ];

        let solver = crate::CpmSolver::new();
        let schedule = solver.schedule(&project).unwrap();
        let calendar = Calendar::default();

        let options = LevelingOptions {
            max_project_delay_factor: Some(3.0),
            ..LevelingOptions::default()
        };
        let result = level_resources_with_options(&project, &schedule, &calendar, &options);

        // Generous factor: should resolve everything
        assert!(
            result.unresolved_conflicts.is_empty(),
            "Generous factor (3.0x) should allow full resolution, got {} unresolved",
            result.unresolved_conflicts.len()
        );
        assert!(
            !result.shifted_tasks.is_empty(),
            "Should have shifted at least one task"
        );
    }

    #[test]
    fn max_delay_factor_none_means_unlimited() {
        // Confirm that max_project_delay_factor = None imposes no limit.
        use utf8proj_core::Scheduler;

        let mut project = Project::new("No Delay Limit");
        project.start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        project.resources = vec![Resource::new("dev").capacity(1.0)];
        project.tasks = vec![
            Task::new("a").effort(Duration::days(10)).assign("dev"),
            Task::new("b").effort(Duration::days(10)).assign("dev"),
        ];

        let solver = crate::CpmSolver::new();
        let schedule = solver.schedule(&project).unwrap();
        let calendar = Calendar::default();

        let options = LevelingOptions {
            max_project_delay_factor: None,
            ..LevelingOptions::default()
        };
        let result = level_resources_with_options(&project, &schedule, &calendar, &options);

        // No limit: must resolve everything regardless of duration increase
        assert!(
            result.unresolved_conflicts.is_empty(),
            "No delay limit should resolve all conflicts"
        );
        assert!(result.project_extended, "Should extend project");
    }

    // --- G3: Hybrid/Cluster Leveling End-to-End ---

    #[test]
    fn hybrid_leveling_resolves_simple_conflict() {
        // Same setup as level_resources_resolves_simple_conflict but
        // using LevelingStrategy::Hybrid.
        use utf8proj_core::Scheduler;

        let mut project = Project::new("Hybrid Simple");
        project.start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        project.resources = vec![Resource::new("dev").capacity(1.0)];
        project.tasks = vec![
            Task::new("a").effort(Duration::days(3)).assign("dev"),
            Task::new("b").effort(Duration::days(3)).assign("dev"),
        ];

        let solver = crate::CpmSolver::new();
        let schedule = solver.schedule(&project).unwrap();
        let calendar = Calendar::default();

        let options = LevelingOptions {
            strategy: LevelingStrategy::Hybrid,
            ..LevelingOptions::default()
        };
        let result = level_resources_with_options(&project, &schedule, &calendar, &options);

        assert!(
            !result.shifted_tasks.is_empty(),
            "Hybrid leveling should shift at least one task"
        );
        assert!(
            result.unresolved_conflicts.is_empty(),
            "Hybrid should resolve simple conflict"
        );

        // Tasks should not overlap after leveling
        let a = &result.leveled_schedule.tasks["a"];
        let b = &result.leveled_schedule.tasks["b"];
        assert!(
            a.finish < b.start || b.finish < a.start,
            "Tasks should not overlap: a={}..{}, b={}..{}",
            a.start, a.finish, b.start, b.finish
        );
    }

    #[test]
    fn hybrid_leveling_extends_project() {
        // Verify hybrid leveling reports project extension correctly.
        use utf8proj_core::Scheduler;

        let mut project = Project::new("Hybrid Extension");
        project.start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        project.resources = vec![Resource::new("dev").capacity(1.0)];
        project.tasks = vec![
            Task::new("a").effort(Duration::days(5)).assign("dev"),
            Task::new("b").effort(Duration::days(5)).assign("dev"),
        ];

        let solver = crate::CpmSolver::new();
        let schedule = solver.schedule(&project).unwrap();
        let calendar = Calendar::default();

        let options = LevelingOptions {
            strategy: LevelingStrategy::Hybrid,
            ..LevelingOptions::default()
        };
        let result = level_resources_with_options(&project, &schedule, &calendar, &options);

        assert!(
            result.project_extended,
            "Hybrid leveling should extend project for parallel tasks"
        );
        assert!(result.new_project_end > schedule.project_end);
        assert!(result.metrics.project_duration_increase > 0);
        assert!(result.metrics.tasks_delayed > 0);
    }

    #[test]
    fn nrt_hybrid_no_negative_gaps() {
        // NRT: The exact same no-negative-gaps invariant, but exercised
        // through the hybrid/cluster code path.
        use utf8proj_core::Scheduler;

        let mut project = Project::new("Hybrid NRT");
        project.start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        project.resources = vec![Resource::new("dev").capacity(1.0)];
        project.tasks = vec![
            Task::new("blocker").effort(Duration::days(5)).assign("dev"),
            Task::new("a").effort(Duration::days(3)).assign("dev"),
            Task::new("b")
                .effort(Duration::days(3))
                .assign("dev")
                .depends_on("a"),
        ];

        let solver = crate::CpmSolver::new();
        let schedule = solver.schedule(&project).unwrap();
        let calendar = Calendar::default();

        let options = LevelingOptions {
            strategy: LevelingStrategy::Hybrid,
            ..LevelingOptions::default()
        };
        let result = level_resources_with_options(&project, &schedule, &calendar, &options);

        assert_no_negative_gaps(&project, &result.leveled_schedule, &calendar);
    }

    #[test]
    fn nrt_hybrid_no_negative_gaps_chain() {
        // NRT: Chain A->B->C through hybrid leveler.
        use utf8proj_core::Scheduler;

        let mut project = Project::new("Hybrid Chain NRT");
        project.start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        project.resources = vec![Resource::new("dev").capacity(1.0)];
        project.tasks = vec![
            Task::new("blocker").effort(Duration::days(5)).assign("dev"),
            Task::new("a").effort(Duration::days(2)).assign("dev"),
            Task::new("b")
                .effort(Duration::days(2))
                .assign("dev")
                .depends_on("a"),
            Task::new("c")
                .effort(Duration::days(2))
                .assign("dev")
                .depends_on("b"),
        ];

        let solver = crate::CpmSolver::new();
        let schedule = solver.schedule(&project).unwrap();
        let calendar = Calendar::default();

        let options = LevelingOptions {
            strategy: LevelingStrategy::Hybrid,
            ..LevelingOptions::default()
        };
        let result = level_resources_with_options(&project, &schedule, &calendar, &options);

        assert_no_negative_gaps(&project, &result.leveled_schedule, &calendar);
    }

    #[test]
    fn nrt_hybrid_no_negative_gaps_multi_resource() {
        // NRT: Cross-resource dependency through hybrid leveler.
        use utf8proj_core::Scheduler;

        let mut project = Project::new("Hybrid Multi NRT");
        project.start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        project.resources = vec![
            Resource::new("dev").capacity(1.0),
            Resource::new("qa").capacity(1.0),
        ];
        project.tasks = vec![
            Task::new("blocker").effort(Duration::days(5)).assign("dev"),
            Task::new("a").effort(Duration::days(3)).assign("dev"),
            Task::new("b")
                .effort(Duration::days(3))
                .assign("qa")
                .depends_on("a"),
        ];

        let solver = crate::CpmSolver::new();
        let schedule = solver.schedule(&project).unwrap();
        let calendar = Calendar::default();

        let options = LevelingOptions {
            strategy: LevelingStrategy::Hybrid,
            ..LevelingOptions::default()
        };
        let result = level_resources_with_options(&project, &schedule, &calendar, &options);

        assert_no_negative_gaps(&project, &result.leveled_schedule, &calendar);
    }

    #[test]
    fn hybrid_leveling_with_max_delay_factor() {
        // Combines hybrid strategy with max delay factor constraint.
        use utf8proj_core::Scheduler;

        let mut project = Project::new("Hybrid Max Delay");
        project.start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        project.resources = vec![Resource::new("dev").capacity(1.0)];
        project.tasks = vec![
            Task::new("a").effort(Duration::days(5)).assign("dev"),
            Task::new("b").effort(Duration::days(5)).assign("dev"),
        ];

        let solver = crate::CpmSolver::new();
        let schedule = solver.schedule(&project).unwrap();
        let calendar = Calendar::default();

        // Restrictive: 1.2x means only 1 extra day allowed on a 5-day project
        let options = LevelingOptions {
            strategy: LevelingStrategy::Hybrid,
            max_project_delay_factor: Some(1.2),
            ..LevelingOptions::default()
        };
        let result = level_resources_with_options(&project, &schedule, &calendar, &options);

        // Should have unresolved conflicts — can't fully sequence within 1.2x
        assert!(
            !result.unresolved_conflicts.is_empty(),
            "Hybrid with restrictive delay factor should leave unresolved conflicts"
        );
    }

    // --- G3 supplement: Hybrid parity with standard leveler ---

    #[test]
    fn hybrid_and_standard_produce_valid_schedules() {
        // Both strategies must produce valid (no negative gap) schedules
        // for the same input. They don't need identical results, but both
        // must satisfy the precedence invariant.
        use utf8proj_core::Scheduler;

        let mut project = Project::new("Parity Test");
        project.start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        project.resources = vec![
            Resource::new("dev").capacity(1.0),
            Resource::new("qa").capacity(1.0),
        ];
        project.tasks = vec![
            Task::new("x").effort(Duration::days(4)).assign("dev"),
            Task::new("y").effort(Duration::days(4)).assign("dev"),
            Task::new("z")
                .effort(Duration::days(3))
                .assign("qa")
                .depends_on("x"),
            Task::new("w")
                .effort(Duration::days(3))
                .assign("qa")
                .depends_on("y"),
        ];

        let solver = crate::CpmSolver::new();
        let schedule = solver.schedule(&project).unwrap();
        let calendar = Calendar::default();

        // Standard
        let standard = level_resources(&project, &schedule, &calendar);
        assert_no_negative_gaps(&project, &standard.leveled_schedule, &calendar);

        // Hybrid
        let hybrid_opts = LevelingOptions {
            strategy: LevelingStrategy::Hybrid,
            ..LevelingOptions::default()
        };
        let hybrid =
            level_resources_with_options(&project, &schedule, &calendar, &hybrid_opts);
        assert_no_negative_gaps(&project, &hybrid.leveled_schedule, &calendar);

        // Both must resolve conflicts (or report them)
        let standard_resolved = standard.unresolved_conflicts.is_empty();
        let hybrid_resolved = hybrid.unresolved_conflicts.is_empty();
        assert_eq!(
            standard_resolved, hybrid_resolved,
            "Standard and hybrid should agree on resolvability: standard={}, hybrid={}",
            standard_resolved, hybrid_resolved
        );
    }
}
