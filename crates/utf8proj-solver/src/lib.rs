//! # utf8proj-solver
//!
//! Scheduling solver implementing Critical Path Method (CPM) and resource leveling.
//!
//! This crate provides:
//! - Forward/backward pass scheduling
//! - Critical path identification
//! - Resource-constrained scheduling
//! - Slack/float calculations
//!
//! ## Example
//!
//! ```rust
//! use utf8proj_core::{Project, Task, Duration};
//! use utf8proj_solver::CpmSolver;
//! use utf8proj_core::Scheduler;
//!
//! let mut project = Project::new("Test");
//! project.tasks.push(Task::new("task1").effort(Duration::days(5)));
//! project.tasks.push(Task::new("task2").effort(Duration::days(3)).depends_on("task1"));
//!
//! let solver = CpmSolver::new();
//! let schedule = solver.schedule(&project).unwrap();
//! assert!(schedule.tasks.contains_key("task1"));
//! ```

use chrono::{NaiveDate, TimeDelta};
use std::collections::{HashMap, VecDeque};

use utf8proj_core::{
    Assignment, Calendar, DependencyType, Duration, Explanation, FeasibilityResult, Project,
    Schedule, ScheduleError, ScheduledTask, Scheduler, Task, TaskConstraint, TaskId, TaskStatus,
};

pub mod bdd;
pub mod cpm;
pub mod dag;
pub mod leveling;

pub use bdd::{BddConflictAnalyzer, BddStats, ConflictAnalysis, ConflictResolution, ResourceConflict, ShiftDirection};
pub use leveling::{
    detect_overallocations, level_resources, LevelingResult, OverallocationPeriod,
    ResourceTimeline, ShiftedTask, UnresolvedConflict,
};

/// CPM-based scheduler
pub struct CpmSolver {
    /// Whether to perform resource leveling
    pub resource_leveling: bool,
}

impl CpmSolver {
    pub fn new() -> Self {
        Self {
            resource_leveling: false,
        }
    }

    /// Create a solver with resource leveling enabled
    pub fn with_leveling() -> Self {
        Self {
            resource_leveling: true,
        }
    }
}

impl Default for CpmSolver {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// Helper Types
// =============================================================================

/// Internal representation of a task for scheduling
#[derive(Clone, Debug)]
struct TaskNode<'a> {
    task: &'a Task,
    /// Duration in working days
    duration_days: i64,
    /// Early Start (days from project start)
    early_start: i64,
    /// Early Finish (days from project start)
    early_finish: i64,
    /// Late Start (days from project start)
    late_start: i64,
    /// Late Finish (days from project start)
    late_finish: i64,
    /// Slack/float in days
    slack: i64,
}

// =============================================================================
// Helper Functions
// =============================================================================

/// Flatten the hierarchical task tree into a HashMap with qualified IDs
///
/// For nested tasks like:
///   phase1 { act1 { sub1 } }
///
/// This produces:
///   "phase1" -> phase1
///   "phase1.act1" -> act1
///   "phase1.act1.sub1" -> sub1
///
/// Also builds a context map for resolving relative dependencies:
///   "phase1.act1" -> "phase1" (parent context for resolving siblings)
fn flatten_tasks_with_prefix<'a>(
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
            flatten_tasks_with_prefix(&task.children, &qualified_id, map, context_map);
        }
    }
}

/// Flatten the hierarchical task tree into a HashMap (convenience wrapper)
fn flatten_tasks<'a>(tasks: &'a [Task], map: &mut HashMap<String, &'a Task>) {
    let mut context_map = HashMap::new();
    flatten_tasks_with_prefix(tasks, "", map, &mut context_map);
}

/// Flatten tasks and return both the task map and context map for dependency resolution
fn flatten_tasks_with_context<'a>(
    tasks: &'a [Task],
) -> (HashMap<String, &'a Task>, HashMap<String, String>) {
    let mut task_map = HashMap::new();
    let mut context_map = HashMap::new();
    flatten_tasks_with_prefix(tasks, "", &mut task_map, &mut context_map);
    (task_map, context_map)
}

/// Build a map from parent qualified ID to list of direct children qualified IDs
fn build_children_map(task_map: &HashMap<String, &Task>) -> HashMap<String, Vec<String>> {
    let mut children_map: HashMap<String, Vec<String>> = HashMap::new();

    for qualified_id in task_map.keys() {
        // Find the parent by removing the last component
        if let Some(dot_pos) = qualified_id.rfind('.') {
            let parent_id = &qualified_id[..dot_pos];
            children_map
                .entry(parent_id.to_string())
                .or_default()
                .push(qualified_id.clone());
        }
    }

    children_map
}

/// Resolve a dependency path to a qualified task ID
///
/// Handles:
/// - Absolute paths: "phase1.act1" -> "phase1.act1"
/// - Relative paths: "act1" (from phase1.act2) -> "phase1.act1"
fn resolve_dependency_path(
    dep_path: &str,
    from_qualified_id: &str,
    context_map: &HashMap<String, String>,
    task_map: &HashMap<String, &Task>,
) -> Option<String> {
    // First, try as absolute path
    if task_map.contains_key(dep_path) {
        return Some(dep_path.to_string());
    }

    // If path contains a dot, it's meant to be absolute - don't try relative resolution
    if dep_path.contains('.') {
        return None;
    }

    // Try relative resolution: look in the same container
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

/// Get the duration of a task in working days
///
/// For effort-driven tasks (PMI "Fixed Work"):
///   Duration = Effort / Total_Resource_Units
///
/// Where Total_Resource_Units is the sum of all assigned resource allocation
/// percentages (e.g., 1.0 = 100%, 0.5 = 50%).
///
/// Examples:
/// - 40h effort with 1 resource @ 100% = 5 days
/// - 40h effort with 1 resource @ 50% = 10 days
/// - 40h effort with 2 resources @ 100% each = 2.5 days
fn get_task_duration_days(task: &Task) -> i64 {
    // If explicit duration is set, use that (Fixed Duration task type)
    if let Some(dur) = task.duration {
        return dur.as_days().ceil() as i64;
    }

    // Effort-driven: Duration = Effort / Total_Resource_Units
    if let Some(effort) = task.effort {
        let total_units: f64 = if task.assigned.is_empty() {
            1.0 // Default: assume 1 resource at 100%
        } else {
            task.assigned.iter().map(|r| r.units as f64).sum()
        };

        // Prevent division by zero
        let effective_units = if total_units > 0.0 { total_units } else { 1.0 };
        return (effort.as_days() / effective_units).ceil() as i64;
    }

    // Milestone or summary task
    0
}

/// Pre-computed mapping from working day index to calendar date
/// This provides O(1) lookup instead of O(days) recalculation
struct WorkingDayCache {
    /// Maps working day index (0, 1, 2, ...) to calendar date
    dates: Vec<NaiveDate>,
}

impl WorkingDayCache {
    /// Build a cache for the given project duration
    fn new(project_start: NaiveDate, max_days: i64, calendar: &Calendar) -> Self {
        let mut dates = Vec::with_capacity((max_days + 1) as usize);
        dates.push(project_start); // Day 0 = project start

        let mut current = project_start;
        for _ in 0..max_days {
            current = current + TimeDelta::days(1);
            while !calendar.is_working_day(current) {
                current = current + TimeDelta::days(1);
            }
            dates.push(current);
        }

        Self { dates }
    }

    /// Get the date for a given working day index (O(1))
    fn get(&self, working_days: i64) -> NaiveDate {
        if working_days <= 0 {
            return self.dates[0];
        }
        let idx = working_days as usize;
        if idx < self.dates.len() {
            self.dates[idx]
        } else {
            // Fallback for days beyond cache (shouldn't happen)
            *self.dates.last().unwrap_or(&self.dates[0])
        }
    }
}

/// Convert a date to working days from project start
fn date_to_working_days(project_start: NaiveDate, target: NaiveDate, calendar: &Calendar) -> i64 {
    if target <= project_start {
        return 0;
    }

    let mut current = project_start;
    let mut working_days = 0i64;

    while current < target {
        current = current + TimeDelta::days(1);
        if calendar.is_working_day(current) {
            working_days += 1;
        }
    }

    working_days
}

/// Add working days to a start date
fn add_working_days(start: NaiveDate, days: i64, calendar: &Calendar) -> NaiveDate {
    if days <= 0 {
        return start;
    }

    let mut current = start;
    let mut remaining = days;

    while remaining > 0 {
        current = current + TimeDelta::days(1);
        if calendar.is_working_day(current) {
            remaining -= 1;
        }
    }

    current
}

/// Result of topological sort including precomputed successor map
struct TopoSortResult {
    /// Tasks in topological order
    sorted_ids: Vec<String>,
    /// Map from task ID to its successors (tasks that depend on it)
    successors: HashMap<String, Vec<String>>,
}

/// Perform topological sort using Kahn's algorithm
/// Returns sorted task IDs and a precomputed successors map
///
/// This ensures:
/// 1. Tasks come after their dependencies (explicit edges)
/// 2. Container tasks come after their children (implicit edges)
fn topological_sort(
    tasks: &HashMap<String, &Task>,
    context_map: &HashMap<String, String>,
) -> Result<TopoSortResult, ScheduleError> {
    // Build children map for container handling
    let children_map = build_children_map(tasks);

    // Build adjacency list, in-degree count, and successors map
    let mut in_degree: HashMap<String, usize> = HashMap::new();
    let mut adjacency: HashMap<String, Vec<String>> = HashMap::new();
    let mut successors: HashMap<String, Vec<String>> = HashMap::new();

    // Initialize all tasks with 0 in-degree and empty successors
    for id in tasks.keys() {
        in_degree.insert(id.clone(), 0);
        adjacency.insert(id.clone(), Vec::new());
        successors.insert(id.clone(), Vec::new());
    }

    // Add implicit edges: children -> container (container comes after children)
    for (container_id, children) in &children_map {
        for child_id in children {
            // child -> container edge
            if let Some(adj) = adjacency.get_mut(child_id) {
                adj.push(container_id.clone());
            }
            if let Some(deg) = in_degree.get_mut(container_id) {
                *deg += 1;
            }
            // Note: container is not a "real" successor for backward pass
        }
    }

    // Build the graph with resolved dependency paths
    for (qualified_id, task) in tasks {
        for dep in &task.depends {
            // Resolve the dependency path (handles both absolute and relative)
            let resolved = resolve_dependency_path(
                &dep.predecessor,
                qualified_id,
                context_map,
                tasks,
            );

            if let Some(pred_id) = resolved {
                // pred_id -> qualified_id (predecessor must come before this task)
                if let Some(adj) = adjacency.get_mut(&pred_id) {
                    adj.push(qualified_id.clone());
                }
                if let Some(deg) = in_degree.get_mut(qualified_id) {
                    *deg += 1;
                }
                // Build successors map: pred_id has qualified_id as successor
                if let Some(succ) = successors.get_mut(&pred_id) {
                    succ.push(qualified_id.clone());
                }
            }
            // If dependency can't be resolved, we skip it (might be external or error)
        }
    }

    // Kahn's algorithm
    let mut queue: VecDeque<String> = VecDeque::new();
    let mut result: Vec<String> = Vec::new();

    // Start with tasks that have no dependencies
    for (id, &degree) in &in_degree {
        if degree == 0 {
            queue.push_back(id.clone());
        }
    }

    while let Some(id) = queue.pop_front() {
        result.push(id.clone());

        if let Some(successors) = adjacency.get(&id) {
            for successor in successors {
                if let Some(deg) = in_degree.get_mut(successor) {
                    *deg -= 1;
                    if *deg == 0 {
                        queue.push_back(successor.clone());
                    }
                }
            }
        }
    }

    // Check for cycles
    if result.len() != tasks.len() {
        let remaining: Vec<_> = tasks
            .keys()
            .filter(|id| !result.contains(id))
            .cloned()
            .collect();
        return Err(ScheduleError::CircularDependency(format!(
            "Cycle detected involving tasks: {:?}",
            remaining
        )));
    }

    Ok(TopoSortResult {
        sorted_ids: result,
        successors,
    })
}

// =============================================================================
// CPM Implementation
// =============================================================================

impl Scheduler for CpmSolver {
    fn schedule(&self, project: &Project) -> Result<Schedule, ScheduleError> {
        // Step 1: Flatten tasks with context for dependency resolution
        let (task_map, context_map) = flatten_tasks_with_context(&project.tasks);

        if task_map.is_empty() {
            // Empty project - return empty schedule
            return Ok(Schedule {
                tasks: HashMap::new(),
                critical_path: Vec::new(),
                project_duration: Duration::zero(),
                project_end: project.start,
                total_cost: None,
            });
        }

        // Step 2: Topological sort (with dependency path resolution)
        let topo_result = topological_sort(&task_map, &context_map)?;
        let sorted_ids = topo_result.sorted_ids;
        let successors_map = topo_result.successors;

        // Step 3: Get calendar (use first calendar or default)
        let calendar = project
            .calendars
            .iter()
            .find(|c| c.id == project.calendar)
            .or_else(|| project.calendars.first())
            .cloned()
            .unwrap_or_default();

        // Step 4: Initialize task nodes
        let mut nodes: HashMap<String, TaskNode> = HashMap::new();
        for id in &sorted_ids {
            let task = task_map[id];
            nodes.insert(
                id.clone(),
                TaskNode {
                    task,
                    duration_days: get_task_duration_days(task),
                    early_start: 0,
                    early_finish: 0,
                    late_start: i64::MAX,
                    late_finish: i64::MAX,
                    slack: 0,
                },
            );
        }

        // Build children map for container date derivation
        let children_map = build_children_map(&task_map);

        // Step 5: Forward pass - calculate ES and EF
        // Because of topological sort, children are processed before their containers
        for id in &sorted_ids {
            let task = task_map[id];

            // Check if this is a container task
            if let Some(children) = children_map.get(id) {
                // Container: derive dates from children
                // All children have already been processed (topological order)
                let mut min_es = i64::MAX;
                let mut max_ef = i64::MIN;

                for child_id in children {
                    if let Some(child_node) = nodes.get(child_id) {
                        min_es = min_es.min(child_node.early_start);
                        max_ef = max_ef.max(child_node.early_finish);
                    }
                }

                if min_es != i64::MAX && max_ef != i64::MIN {
                    if let Some(node) = nodes.get_mut(id) {
                        node.early_start = min_es;
                        node.early_finish = max_ef;
                        node.duration_days = max_ef - min_es;
                    }
                }
            } else {
                // Leaf task: normal forward pass logic
                let duration = nodes[id].duration_days;

                // ES = max of all dependency constraints, or 0 if no predecessors
                // Dependency types:
                //   FS: B.start >= A.finish + lag
                //   SS: B.start >= A.start + lag
                //   FF: B.finish >= A.finish + lag → B.start >= A.finish + lag - B.duration
                //   SF: B.finish >= A.start + lag → B.start >= A.start + lag - B.duration
                let mut es = 0i64;
                for dep in &task.depends {
                    // Resolve the dependency path to get the qualified ID
                    let resolved = resolve_dependency_path(
                        &dep.predecessor,
                        id,
                        &context_map,
                        &task_map,
                    );
                    if let Some(pred_id) = resolved {
                        if let Some(pred_node) = nodes.get(&pred_id) {
                            let lag = dep.lag.map(|d| d.as_days() as i64).unwrap_or(0);

                            let constraint_es = match dep.dep_type {
                                DependencyType::FinishToStart => {
                                    // B.start >= A.finish + lag
                                    // For positive lag: count from first free day (early_finish)
                                    // For negative lag: count from last working day (early_finish - 1)
                                    if lag >= 0 {
                                        pred_node.early_finish + lag
                                    } else {
                                        (pred_node.early_finish - 1 + lag).max(0)
                                    }
                                }
                                DependencyType::StartToStart => {
                                    // B.start >= A.start + lag
                                    pred_node.early_start + lag
                                }
                                DependencyType::FinishToFinish => {
                                    // B.finish >= A.finish + lag
                                    // B.start >= A.finish + lag - B.duration
                                    (pred_node.early_finish + lag - duration).max(0)
                                }
                                DependencyType::StartToFinish => {
                                    // B.finish >= A.start + lag
                                    // B.start >= A.start + lag - B.duration
                                    (pred_node.early_start + lag - duration).max(0)
                                }
                            };
                            es = es.max(constraint_es);
                        }
                    }
                }

                // Also consider MustStartOn constraints
                for constraint in &task.constraints {
                    if let TaskConstraint::MustStartOn(date) = constraint {
                        // Convert date to working days from project start
                        let constraint_days = date_to_working_days(project.start, *date, &calendar);
                        es = es.max(constraint_days);
                    }
                }

                // EF = ES + duration
                let ef = es + duration;

                if let Some(node) = nodes.get_mut(id) {
                    node.early_start = es;
                    node.early_finish = ef;
                }
            }
        }

        // Step 6: Find project end (max EF)
        let project_end_days = nodes.values().map(|n| n.early_finish).max().unwrap_or(0);

        // Build working day cache for O(1) date lookups
        let working_day_cache = WorkingDayCache::new(project.start, project_end_days, &calendar);

        // Step 7: Backward pass - calculate LS and LF
        // Process in reverse topological order (leaf tasks first, then containers)
        // Note: In reverse order, containers come first, but we skip them and handle after
        for id in sorted_ids.iter().rev() {
            // Skip containers - they'll be handled in Step 7b
            if children_map.contains_key(id) {
                continue;
            }

            let duration = nodes[id].duration_days;

            // Get successors from precomputed map (O(1) lookup instead of O(n) scan)
            let successors = successors_map.get(id);

            // LF = min(LS of all successors), or project_end if no successors
            let lf = match successors {
                Some(succs) if !succs.is_empty() => succs
                    .iter()
                    .filter_map(|s| nodes.get(s).map(|n| n.late_start))
                    .min()
                    .unwrap_or(project_end_days),
                _ => project_end_days,
            };

            // LS = LF - duration
            let ls = lf - duration;

            // Slack = LS - ES (or LF - EF, they should be equal)
            let slack = ls - nodes[id].early_start;

            if let Some(node) = nodes.get_mut(id) {
                node.late_start = ls;
                node.late_finish = lf;
                node.slack = slack;
            }
        }

        // Step 7b: Derive container late dates from children (process deepest first)
        let mut container_ids: Vec<&String> = children_map.keys().collect();
        container_ids.sort_by(|a, b| {
            let depth_a = a.matches('.').count();
            let depth_b = b.matches('.').count();
            depth_b.cmp(&depth_a) // Deepest first
        });

        for container_id in container_ids {
            if let Some(children) = children_map.get(container_id) {
                let mut min_ls = i64::MAX;
                let mut max_lf = i64::MIN;

                for child_id in children {
                    if let Some(child_node) = nodes.get(child_id) {
                        min_ls = min_ls.min(child_node.late_start);
                        max_lf = max_lf.max(child_node.late_finish);
                    }
                }

                if min_ls != i64::MAX && max_lf != i64::MIN {
                    if let Some(container_node) = nodes.get_mut(container_id) {
                        container_node.late_start = min_ls;
                        container_node.late_finish = max_lf;
                        container_node.slack = min_ls - container_node.early_start;
                    }
                }
            }
        }

        // Step 8: Identify critical path (tasks with zero slack)
        // Build position map for O(1) lookup during sort
        let position_map: HashMap<&String, usize> = sorted_ids
            .iter()
            .enumerate()
            .map(|(i, id)| (id, i))
            .collect();

        let mut critical_path: Vec<TaskId> = nodes
            .iter()
            .filter(|(_, node)| node.slack == 0 && node.duration_days > 0)
            .map(|(id, _)| id.clone())
            .collect();

        // Sort critical path in topological order using O(1) position lookup
        critical_path.sort_by_key(|id| position_map.get(id).copied().unwrap_or(0));

        // Step 9: Build ScheduledTask entries
        let mut scheduled_tasks: HashMap<TaskId, ScheduledTask> = HashMap::new();

        for (id, node) in &nodes {
            let start_date = working_day_cache.get(node.early_start);
            // Finish date is the last day of work, not the day after
            // So for a 20-day task starting Feb 03, finish is Feb 28 (day 20), not Mar 03
            let finish_date = if node.duration_days > 0 {
                // early_finish - 1 because finish is inclusive (last day of work)
                working_day_cache.get(node.early_finish - 1)
            } else {
                start_date // Milestone
            };

            // Build assignments (simplified - one assignment per assigned resource)
            let assignments: Vec<Assignment> = node
                .task
                .assigned
                .iter()
                .map(|res_ref| Assignment {
                    resource_id: res_ref.resource_id.clone(),
                    start: start_date,
                    finish: finish_date,
                    units: res_ref.units,
                    cost: None, // Cost calculation would go here
                })
                .collect();

            // Progress tracking calculations
            let task = node.task;
            let percent_complete = task.effective_percent_complete();
            let remaining = task.remaining_duration();
            let status = task.derived_status();

            // Forecast start: use actual_start if available, otherwise planned start
            let forecast_start = task.actual_start.unwrap_or(start_date);

            // Forecast finish: based on forecast_start + remaining_duration
            // If task is complete, use actual_finish or finish_date
            let forecast_finish = if status == TaskStatus::Complete {
                task.actual_finish.unwrap_or(finish_date)
            } else if remaining.minutes > 0 {
                // Add remaining working days to forecast_start
                let remaining_days = remaining.as_days().ceil() as i64;
                add_working_days(forecast_start, remaining_days - 1, &calendar)
            } else {
                forecast_start // Milestone or complete
            };

            scheduled_tasks.insert(
                id.clone(),
                ScheduledTask {
                    task_id: id.clone(),
                    start: start_date,
                    finish: finish_date,
                    duration: Duration::days(node.duration_days),
                    assignments,
                    slack: Duration::days(node.slack),
                    is_critical: node.slack == 0 && node.duration_days > 0,
                    early_start: working_day_cache.get(node.early_start),
                    early_finish: if node.duration_days > 0 {
                        working_day_cache.get(node.early_finish - 1)
                    } else {
                        working_day_cache.get(node.early_finish)
                    },
                    late_start: working_day_cache.get(node.late_start),
                    late_finish: if node.duration_days > 0 {
                        working_day_cache.get(node.late_finish - 1)
                    } else {
                        working_day_cache.get(node.late_finish)
                    },
                    // Progress tracking fields
                    forecast_start,
                    forecast_finish,
                    remaining_duration: remaining,
                    percent_complete,
                    status,
                },
            );
        }

        // Step 10: Build final schedule
        // project_end is the last working day of the project
        let project_end_date = if project_end_days > 0 {
            working_day_cache.get(project_end_days - 1)
        } else {
            project.start
        };

        let schedule = Schedule {
            tasks: scheduled_tasks,
            critical_path,
            project_duration: Duration::days(project_end_days),
            project_end: project_end_date,
            total_cost: None, // Cost calculation would go here
        };

        // Step 11: Apply resource leveling if enabled
        if self.resource_leveling {
            let result = level_resources(project, &schedule, &calendar);
            Ok(result.schedule)
        } else {
            Ok(schedule)
        }
    }

    fn is_feasible(&self, project: &Project) -> FeasibilityResult {
        // Try to schedule and check for errors
        let (task_map, context_map) = flatten_tasks_with_context(&project.tasks);

        match topological_sort(&task_map, &context_map) {
            Ok(_topo_result) => FeasibilityResult {
                feasible: true,
                conflicts: vec![],
                suggestions: vec![],
            },
            Err(e) => FeasibilityResult {
                feasible: false,
                conflicts: vec![utf8proj_core::Conflict {
                    conflict_type: utf8proj_core::ConflictType::CircularDependency,
                    description: e.to_string(),
                    involved_tasks: vec![],
                    involved_resources: vec![],
                }],
                suggestions: vec![],
            },
        }
    }

    fn explain(&self, project: &Project, task_id: &TaskId) -> Explanation {
        // Try to find the task and explain its scheduling
        let mut task_map: HashMap<String, &Task> = HashMap::new();
        flatten_tasks(&project.tasks, &mut task_map);

        if let Some(task) = task_map.get(task_id) {
            let constraints: Vec<String> = task
                .depends
                .iter()
                .map(|d| format!("Depends on: {}", d.predecessor))
                .collect();

            Explanation {
                task_id: task_id.clone(),
                reason: if task.depends.is_empty() {
                    "Scheduled at project start (no dependencies)".into()
                } else {
                    format!(
                        "Scheduled after predecessors: {}",
                        task.depends
                            .iter()
                            .map(|d| d.predecessor.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                },
                constraints_applied: constraints,
                alternatives_considered: vec![],
            }
        } else {
            Explanation {
                task_id: task_id.clone(),
                reason: "Task not found".into(),
                constraints_applied: vec![],
                alternatives_considered: vec![],
            }
        }
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use utf8proj_core::{Resource, Task};

    fn make_test_project() -> Project {
        let mut project = Project::new("Test Project");
        project.start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap(); // Monday

        // Simple linear chain: design -> implement -> test
        project.tasks = vec![
            Task::new("design")
                .name("Design Phase")
                .effort(Duration::days(5)),
            Task::new("implement")
                .name("Implementation")
                .effort(Duration::days(10))
                .depends_on("design"),
            Task::new("test")
                .name("Testing")
                .effort(Duration::days(3))
                .depends_on("implement"),
        ];

        project.resources = vec![Resource::new("dev").name("Developer")];

        project
    }

    #[test]
    fn solver_creation() {
        let solver = CpmSolver::new();
        assert!(!solver.resource_leveling);
    }

    #[test]
    fn schedule_empty_project() {
        let project = Project::new("Empty");
        let solver = CpmSolver::new();
        let schedule = solver.schedule(&project).unwrap();

        assert!(schedule.tasks.is_empty());
        assert!(schedule.critical_path.is_empty());
        assert_eq!(schedule.project_duration, Duration::zero());
    }

    #[test]
    fn schedule_single_task() {
        let mut project = Project::new("Single Task");
        project.start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        project.tasks = vec![Task::new("task1").effort(Duration::days(5))];

        let solver = CpmSolver::new();
        let schedule = solver.schedule(&project).unwrap();

        assert_eq!(schedule.tasks.len(), 1);
        assert!(schedule.tasks.contains_key("task1"));

        let task = &schedule.tasks["task1"];
        assert_eq!(task.start, project.start);
        assert_eq!(task.duration, Duration::days(5));
        assert!(task.is_critical);
    }

    #[test]
    fn schedule_linear_chain() {
        let project = make_test_project();
        let solver = CpmSolver::new();
        let schedule = solver.schedule(&project).unwrap();

        // All tasks should be scheduled
        assert_eq!(schedule.tasks.len(), 3);

        // Project duration: 5 + 10 + 3 = 18 days
        assert_eq!(schedule.project_duration, Duration::days(18));

        // All tasks in a linear chain are critical
        assert!(schedule.tasks["design"].is_critical);
        assert!(schedule.tasks["implement"].is_critical);
        assert!(schedule.tasks["test"].is_critical);

        // Check ordering: design starts at day 0
        assert_eq!(schedule.tasks["design"].early_start, project.start);

        // implement starts after design (day 5)
        let implement_start = schedule.tasks["implement"].early_start;
        let design_finish = schedule.tasks["design"].early_finish;
        assert!(implement_start >= design_finish);
    }

    #[test]
    fn schedule_parallel_tasks() {
        let mut project = Project::new("Parallel");
        project.start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();

        // Two parallel paths:
        // design (5d) -> implement (10d)
        // docs (3d) -> review (2d)
        // Both converge on: deploy (depends on implement and review)
        project.tasks = vec![
            Task::new("design").effort(Duration::days(5)),
            Task::new("implement")
                .effort(Duration::days(10))
                .depends_on("design"),
            Task::new("docs").effort(Duration::days(3)),
            Task::new("review")
                .effort(Duration::days(2))
                .depends_on("docs"),
            Task::new("deploy")
                .effort(Duration::days(1))
                .depends_on("implement")
                .depends_on("review"),
        ];

        let solver = CpmSolver::new();
        let schedule = solver.schedule(&project).unwrap();

        // Critical path: design -> implement -> deploy (5 + 10 + 1 = 16 days)
        // Non-critical: docs -> review (3 + 2 = 5 days)
        assert_eq!(schedule.project_duration, Duration::days(16));

        // design, implement, deploy should be critical
        assert!(schedule.tasks["design"].is_critical);
        assert!(schedule.tasks["implement"].is_critical);
        assert!(schedule.tasks["deploy"].is_critical);

        // docs and review have slack
        assert!(!schedule.tasks["docs"].is_critical);
        assert!(!schedule.tasks["review"].is_critical);
        assert!(schedule.tasks["docs"].slack.minutes > 0);
    }

    #[test]
    fn detect_circular_dependency() {
        let mut project = Project::new("Circular");
        project.tasks = vec![
            Task::new("a").depends_on("c"),
            Task::new("b").depends_on("a"),
            Task::new("c").depends_on("b"),
        ];

        let solver = CpmSolver::new();
        let result = solver.schedule(&project);

        assert!(result.is_err());
        if let Err(ScheduleError::CircularDependency(msg)) = result {
            assert!(msg.contains("Cycle"));
        } else {
            panic!("Expected CircularDependency error");
        }
    }

    #[test]
    fn feasibility_check() {
        let project = make_test_project();
        let solver = CpmSolver::new();
        let result = solver.is_feasible(&project);

        assert!(result.feasible);
        assert!(result.conflicts.is_empty());
    }

    #[test]
    fn explain_task() {
        let project = make_test_project();
        let solver = CpmSolver::new();

        let explanation = solver.explain(&project, &"implement".to_string());
        assert_eq!(explanation.task_id, "implement");
        assert!(explanation.reason.contains("design"));
    }

    #[test]
    fn milestone_has_zero_duration() {
        let mut project = Project::new("Milestone Test");
        project.start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        project.tasks = vec![
            Task::new("work").effort(Duration::days(5)),
            Task::new("done").milestone().depends_on("work"),
        ];

        let solver = CpmSolver::new();
        let schedule = solver.schedule(&project).unwrap();

        assert_eq!(schedule.tasks["done"].duration, Duration::zero());
        assert_eq!(schedule.tasks["done"].start, schedule.tasks["done"].finish);
    }

    #[test]
    fn nested_tasks_are_flattened() {
        let mut project = Project::new("Nested");
        project.start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();

        // Parent task with children
        // Note: "implement" depends on "design" (relative sibling reference)
        project.tasks = vec![Task::new("phase1")
            .child(Task::new("design").effort(Duration::days(3)))
            .child(
                Task::new("implement")
                    .effort(Duration::days(5))
                    .depends_on("design"), // Relative reference to sibling
            )];

        let solver = CpmSolver::new();
        let schedule = solver.schedule(&project).unwrap();

        // Tasks are stored with qualified IDs (parent.child)
        assert!(schedule.tasks.contains_key("phase1"));
        assert!(schedule.tasks.contains_key("phase1.design"));
        assert!(schedule.tasks.contains_key("phase1.implement"));

        // Verify dependency was resolved: implement starts after design
        let design_task = &schedule.tasks["phase1.design"];
        let implement_task = &schedule.tasks["phase1.implement"];

        println!("design: start={}, finish={}", design_task.start, design_task.finish);
        println!("implement: start={}, finish={}", implement_task.start, implement_task.finish);

        // implement should start on or after design finishes
        // Note: finish is inclusive (last day of work), so implement can start on next working day
        assert!(
            implement_task.start > design_task.finish,
            "implement should start after design finishes"
        );
    }

    // =========================================================================
    // Effort-Driven Duration Tests (PMI Compliance)
    // =========================================================================

    #[test]
    fn effort_with_no_resource_assumes_100_percent() {
        // No resources assigned = assume 1 resource at 100%
        let mut project = Project::new("Test");
        project.start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        project.tasks = vec![Task::new("work").effort(Duration::days(5))];

        let solver = CpmSolver::new();
        let schedule = solver.schedule(&project).unwrap();

        // 5 days effort / 1.0 units = 5 days
        assert_eq!(schedule.tasks["work"].duration.as_days(), 5.0);
    }

    #[test]
    fn effort_with_full_allocation() {
        // 1 resource at 100% allocation
        let mut project = Project::new("Test");
        project.start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        project.resources = vec![Resource::new("dev")];
        project.tasks = vec![Task::new("work")
            .effort(Duration::days(5))
            .assign("dev")]; // 100% by default

        let solver = CpmSolver::new();
        let schedule = solver.schedule(&project).unwrap();

        // 5 days effort / 1.0 units = 5 days
        assert_eq!(schedule.tasks["work"].duration.as_days(), 5.0);
    }

    #[test]
    fn effort_with_partial_allocation() {
        // 1 resource at 50% allocation = duration doubles
        let mut project = Project::new("Test");
        project.start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        project.resources = vec![Resource::new("dev")];
        project.tasks = vec![Task::new("work")
            .effort(Duration::days(5))
            .assign_with_units("dev", 0.5)]; // 50%

        let solver = CpmSolver::new();
        let schedule = solver.schedule(&project).unwrap();

        // 5 days effort / 0.5 units = 10 days
        assert_eq!(schedule.tasks["work"].duration.as_days(), 10.0);
    }

    #[test]
    fn effort_with_multiple_resources() {
        // 2 resources at 100% each = duration halves
        let mut project = Project::new("Test");
        project.start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        project.resources = vec![Resource::new("dev1"), Resource::new("dev2")];
        project.tasks = vec![Task::new("work")
            .effort(Duration::days(10))
            .assign("dev1")
            .assign("dev2")]; // 100% + 100% = 200%

        let solver = CpmSolver::new();
        let schedule = solver.schedule(&project).unwrap();

        // 10 days effort / 2.0 units = 5 days
        assert_eq!(schedule.tasks["work"].duration.as_days(), 5.0);
    }

    #[test]
    fn effort_with_mixed_allocations() {
        // 1 resource at 100% + 1 at 50% = 150% total
        let mut project = Project::new("Test");
        project.start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        project.resources = vec![Resource::new("dev1"), Resource::new("dev2")];
        project.tasks = vec![Task::new("work")
            .effort(Duration::days(15))
            .assign("dev1") // 100%
            .assign_with_units("dev2", 0.5)]; // 50%

        let solver = CpmSolver::new();
        let schedule = solver.schedule(&project).unwrap();

        // 15 days effort / 1.5 units = 10 days
        assert_eq!(schedule.tasks["work"].duration.as_days(), 10.0);
    }

    #[test]
    fn fixed_duration_ignores_allocation() {
        // Explicit duration overrides effort-based calculation
        let mut project = Project::new("Test");
        project.start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        project.resources = vec![Resource::new("dev")];
        project.tasks = vec![Task::new("meeting")
            .duration(Duration::days(1)) // Fixed 1 day
            .assign_with_units("dev", 0.25)]; // 25% shouldn't matter

        let solver = CpmSolver::new();
        let schedule = solver.schedule(&project).unwrap();

        // Duration is fixed at 1 day regardless of allocation
        assert_eq!(schedule.tasks["meeting"].duration.as_days(), 1.0);
    }

    #[test]
    fn effort_chain_with_different_allocations() {
        // Chain of tasks with varying allocations
        let mut project = Project::new("Test");
        project.start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        project.resources = vec![Resource::new("dev")];
        project.tasks = vec![
            Task::new("phase1")
                .effort(Duration::days(5))
                .assign("dev"), // 100% -> 5 days
            Task::new("phase2")
                .effort(Duration::days(5))
                .assign_with_units("dev", 0.5) // 50% -> 10 days
                .depends_on("phase1"),
        ];

        let solver = CpmSolver::new();
        let schedule = solver.schedule(&project).unwrap();

        // Total project duration: 5 + 10 = 15 days
        assert_eq!(schedule.project_duration.as_days(), 15.0);
        assert_eq!(schedule.tasks["phase1"].duration.as_days(), 5.0);
        assert_eq!(schedule.tasks["phase2"].duration.as_days(), 10.0);
    }

    #[test]
    fn solver_default() {
        let solver = CpmSolver::default();
        assert!(!solver.resource_leveling);
    }

    #[test]
    fn explain_nonexistent_task() {
        let project = make_test_project();
        let solver = CpmSolver::new();

        let explanation = solver.explain(&project, &"nonexistent".to_string());
        assert_eq!(explanation.task_id, "nonexistent");
        assert!(explanation.reason.contains("not found"));
    }

    #[test]
    fn feasibility_check_circular_dependency() {
        use utf8proj_core::Scheduler;

        let mut project = Project::new("Circular");
        project.start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        project.tasks = vec![
            Task::new("a").effort(Duration::days(1)).depends_on("c"),
            Task::new("b").effort(Duration::days(1)).depends_on("a"),
            Task::new("c").effort(Duration::days(1)).depends_on("b"),
        ];

        let solver = CpmSolver::new();
        let result = solver.is_feasible(&project);

        assert!(!result.feasible);
        assert!(!result.conflicts.is_empty());
    }

    #[test]
    fn isolated_task_no_dependencies() {
        // Task with no dependencies and nothing depends on it
        let mut project = Project::new("Isolated");
        project.start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        project.tasks = vec![
            Task::new("alone").effort(Duration::days(3)),
        ];

        let solver = CpmSolver::new();
        let schedule = solver.schedule(&project).unwrap();

        assert!(schedule.tasks.contains_key("alone"));
        assert_eq!(schedule.tasks["alone"].duration, Duration::days(3));
    }

    #[test]
    fn deeply_nested_tasks() {
        // Multiple levels of nesting
        let mut project = Project::new("Deep Nesting");
        project.start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        project.tasks = vec![
            Task::new("level1")
                .child(
                    Task::new("level2")
                        .child(
                            Task::new("level3")
                                .child(Task::new("leaf").effort(Duration::days(2)))
                        )
                ),
        ];

        let solver = CpmSolver::new();
        let schedule = solver.schedule(&project).unwrap();

        // All levels should be in schedule
        assert!(schedule.tasks.contains_key("level1"));
        assert!(schedule.tasks.contains_key("level1.level2"));
        assert!(schedule.tasks.contains_key("level1.level2.level3"));
        assert!(schedule.tasks.contains_key("level1.level2.level3.leaf"));
    }

    #[test]
    fn explain_task_with_no_dependencies() {
        let mut project = Project::new("Simple");
        project.start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        project.tasks = vec![
            Task::new("standalone").effort(Duration::days(5)),
        ];

        let solver = CpmSolver::new();
        let explanation = solver.explain(&project, &"standalone".to_string());

        assert!(explanation.reason.contains("project start"));
        assert!(explanation.constraints_applied.is_empty());
    }

    #[test]
    fn schedule_with_dependency_on_container() {
        // Task that depends on a container (should expand to all children)
        let mut project = Project::new("Container Dep");
        project.start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        project.tasks = vec![
            Task::new("phase1")
                .child(Task::new("a").effort(Duration::days(3)))
                .child(Task::new("b").effort(Duration::days(2)).depends_on("a")),
            Task::new("phase2")
                .effort(Duration::days(4))
                .depends_on("phase1"), // Depends on container
        ];

        let solver = CpmSolver::new();
        let schedule = solver.schedule(&project).unwrap();

        // phase2 should start after phase1.b finishes
        let phase1_b = &schedule.tasks["phase1.b"];
        let phase2 = &schedule.tasks["phase2"];
        assert!(phase2.start > phase1_b.finish);
    }

    #[test]
    fn schedule_with_relative_sibling_dependency() {
        // Sibling tasks referencing each other without qualified paths
        let mut project = Project::new("Sibling Deps");
        project.start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        project.tasks = vec![
            Task::new("container")
                .child(Task::new("first").effort(Duration::days(3)))
                .child(Task::new("second").effort(Duration::days(2)).depends_on("first"))
                .child(Task::new("third").effort(Duration::days(1)).depends_on("second")),
        ];

        let solver = CpmSolver::new();
        let schedule = solver.schedule(&project).unwrap();

        // Chain should be scheduled correctly
        assert!(schedule.tasks["container.second"].start > schedule.tasks["container.first"].finish);
        assert!(schedule.tasks["container.third"].start > schedule.tasks["container.second"].finish);
    }

    #[test]
    fn working_day_cache_beyond_limit() {
        // Test with a project that exceeds typical cache size
        let mut project = Project::new("Long Project");
        project.start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        project.tasks = vec![
            Task::new("long_task").duration(Duration::days(500)), // Very long
        ];

        let solver = CpmSolver::new();
        let schedule = solver.schedule(&project).unwrap();

        // Should still schedule without error
        assert!(schedule.tasks.contains_key("long_task"));
        assert_eq!(schedule.tasks["long_task"].duration, Duration::days(500));
    }
}
