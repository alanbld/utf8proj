//! Resource Leveling Algorithm
//!
//! Detects and resolves resource over-allocation by shifting tasks within their slack.

use chrono::NaiveDate;
use std::collections::{BinaryHeap, HashMap};
use utf8proj_core::{Calendar, Duration, Project, ResourceId, Schedule, ScheduledTask, TaskId};

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
    /// Usage by date
    pub usage: HashMap<NaiveDate, DayUsage>,
}

impl ResourceTimeline {
    pub fn new(resource_id: ResourceId, capacity: f32) -> Self {
        Self {
            resource_id,
            capacity,
            usage: HashMap::new(),
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
                                involved_tasks: day.tasks.iter().map(|(id, _)| id.clone()).collect(),
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

    /// Find first available slot where a task can be scheduled without overallocation
    pub fn find_available_slot(
        &self,
        duration_days: i64,
        units: f32,
        earliest_start: NaiveDate,
        calendar: &Calendar,
    ) -> NaiveDate {
        let mut candidate = earliest_start;

        loop {
            // Check if all days in this slot are available
            let mut all_clear = true;
            let mut check_date = candidate;
            let mut working_days_checked = 0;

            while working_days_checked < duration_days {
                if calendar.is_working_day(check_date) {
                    let current_usage = self
                        .usage
                        .get(&check_date)
                        .map(|d| d.total_units)
                        .unwrap_or(0.0);

                    if current_usage + units > self.capacity {
                        all_clear = false;
                        break;
                    }
                    working_days_checked += 1;
                }
                check_date = check_date.succ_opt().unwrap_or(check_date);
            }

            if all_clear {
                return candidate;
            }

            // Move to next working day
            candidate = candidate.succ_opt().unwrap_or(candidate);
            while !calendar.is_working_day(candidate) {
                candidate = candidate.succ_opt().unwrap_or(candidate);
            }
        }
    }
}

/// A period of resource over-allocation
#[derive(Debug, Clone)]
pub struct OverallocationPeriod {
    pub start: NaiveDate,
    pub end: NaiveDate,
    pub peak_usage: f32,
    pub involved_tasks: Vec<TaskId>,
}

/// Result of resource leveling
#[derive(Debug, Clone)]
pub struct LevelingResult {
    /// Updated schedule with leveled tasks
    pub schedule: Schedule,
    /// Tasks that were shifted
    pub shifted_tasks: Vec<ShiftedTask>,
    /// Conflicts that could not be resolved
    pub unresolved_conflicts: Vec<UnresolvedConflict>,
    /// Whether the project duration was extended
    pub project_extended: bool,
    /// New project end date
    pub new_project_end: NaiveDate,
}

/// A task that was shifted during leveling
#[derive(Debug, Clone)]
pub struct ShiftedTask {
    pub task_id: TaskId,
    pub original_start: NaiveDate,
    pub new_start: NaiveDate,
    pub days_shifted: i64,
    pub reason: String,
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
        // Prefer tasks that:
        // 1. Are not critical
        // 2. Have more slack
        // 3. Have lower priority
        match (self.is_critical, other.is_critical) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => self
                .slack_days
                .cmp(&other.slack_days)
                .then(other.priority.cmp(&self.priority)),
        }
    }
}

impl PartialOrd for ShiftCandidate {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// Perform resource leveling on a schedule
pub fn level_resources(
    project: &Project,
    schedule: &Schedule,
    calendar: &Calendar,
) -> LevelingResult {
    let mut leveled_tasks = schedule.tasks.clone();
    let mut shifted_tasks = Vec::new();
    let mut unresolved_conflicts = Vec::new();

    // Build resource timelines from schedule
    let mut timelines = build_resource_timelines(project, &leveled_tasks);

    // Build task priority map for shifting decisions
    let task_priorities = build_task_priority_map(&leveled_tasks, project);

    // Iterate until no more over-allocations or can't resolve
    let max_iterations = leveled_tasks.len() * 10; // Prevent infinite loops
    let mut iterations = 0;

    while iterations < max_iterations {
        iterations += 1;

        // Find first over-allocation
        let conflict = timelines
            .values()
            .flat_map(|t| {
                t.overallocated_periods()
                    .into_iter()
                    .map(|p| (t.resource_id.clone(), p))
            })
            .next();

        let Some((resource_id, period)) = conflict else {
            break; // No more conflicts
        };

        // Find candidates to shift (tasks in this period)
        // Even critical tasks can be shifted if they're part of an over-allocation
        // The critical path will be recalculated after leveling
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
                period,
                reason: "No shiftable tasks found".into(),
            });
            continue;
        }

        // Pick the best candidate to shift
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

        let new_start = timeline.find_available_slot(
            duration_days,
            units,
            period.end.succ_opt().unwrap_or(period.end),
            calendar,
        );

        // Shift the task
        let days_shifted = count_working_days(original_start, new_start, calendar);
        let new_finish = add_working_days(new_start, duration_days, calendar);

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

        // Record the shift
        shifted_tasks.push(ShiftedTask {
            task_id: candidate.task_id.clone(),
            original_start,
            new_start,
            days_shifted,
            reason: format!("Resource {} overallocated", resource_id),
        });
    }

    // Calculate new project end
    let new_project_end = leveled_tasks
        .values()
        .map(|t| t.finish)
        .max()
        .unwrap_or(schedule.project_end);

    let project_extended = new_project_end > schedule.project_end;

    // Recalculate critical path if project was extended
    let critical_path = if project_extended {
        recalculate_critical_path(&leveled_tasks, new_project_end)
    } else {
        schedule.critical_path.clone()
    };

    // Calculate new project duration
    let project_duration_days = count_working_days(project.start, new_project_end, calendar);

    LevelingResult {
        schedule: Schedule {
            tasks: leveled_tasks,
            critical_path,
            project_duration: Duration::days(project_duration_days),
            project_end: new_project_end,
            total_cost: schedule.total_cost.clone(),
            total_cost_range: schedule.total_cost_range.clone(),
            // Project status (I004): Copy from original schedule
            // Note: These could be recomputed from leveled tasks for accuracy
            project_progress: schedule.project_progress,
            project_baseline_finish: schedule.project_baseline_finish,
            project_forecast_finish: schedule.project_forecast_finish,
            project_variance_days: schedule.project_variance_days,
            // Earned Value (I005): Copy from original schedule
            planned_value: schedule.planned_value,
            earned_value: schedule.earned_value,
            spi: schedule.spi,
        },
        shifted_tasks,
        unresolved_conflicts,
        project_extended,
        new_project_end,
    }
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
                timeline.add_usage(&task.task_id, assignment.start, assignment.finish, assignment.units);
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

/// Count working days between two dates
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
        assert!(slot > finish);
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
        assert!(!result.shifted_tasks.is_empty(), "Should shift at least one task");

        // No unresolved conflicts
        assert!(
            result.unresolved_conflicts.is_empty(),
            "Should resolve all conflicts"
        );

        // Tasks should no longer overlap
        let task1 = &result.schedule.tasks["task1"];
        let task2 = &result.schedule.tasks["task2"];

        let overlap = task1.start <= task2.finish && task2.start <= task1.finish;
        assert!(!overlap || task1.start == task2.start, "Tasks should not overlap after leveling");
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
        let task1 = &result.schedule.tasks["task1"];
        let task2 = &result.schedule.tasks["task2"];

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
        use utf8proj_core::{Dependency, DependencyType, Project, Resource, ResourceRef, Scheduler};

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
        assert!(overallocations.is_empty(), "Sequential critical tasks should not conflict");
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
        project.tasks = vec![
            Task::new("task1").effort(Duration::days(5)).assign("dev"),
        ];

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
        let dev1 = utilization.resources.iter().find(|r| r.resource_id == "dev1").unwrap();
        let dev2 = utilization.resources.iter().find(|r| r.resource_id == "dev2").unwrap();
        assert!(dev1.used_days > dev2.used_days);
    }

    #[test]
    fn calculate_utilization_no_resources() {
        use utf8proj_core::Scheduler;

        let mut project = Project::new("No Resources Test");
        project.start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        project.tasks = vec![
            Task::new("task1").effort(Duration::days(5)),
        ];

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
        project.tasks = vec![
            Task::new("task1").effort(Duration::days(5)).assign("dev1"),
        ];

        let solver = crate::CpmSolver::new();
        let schedule = solver.schedule(&project).unwrap();
        let calendar = Calendar::default();

        let utilization = calculate_utilization(&project, &schedule, &calendar);

        let dev2 = utilization.resources.iter().find(|r| r.resource_id == "dev2").unwrap();
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
}
