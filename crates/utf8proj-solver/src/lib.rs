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
    Assignment, Calendar, CostRange, DependencyType, Duration, Explanation, FeasibilityResult,
    Money, Project, RateRange, ResourceProfile, Schedule, ScheduleError, ScheduledTask, Scheduler,
    Task, TaskConstraint, TaskId, TaskStatus,
    // Diagnostics
    Diagnostic, DiagnosticCode, DiagnosticEmitter,
};
use rust_decimal::prelude::FromPrimitive;
use rust_decimal::Decimal;
use std::path::PathBuf;

pub mod bdd;
pub mod cpm;
pub mod dag;
pub mod leveling;

pub use bdd::{BddConflictAnalyzer, BddStats, ConflictAnalysis, ConflictResolution, ResourceConflict, ShiftDirection};
pub use leveling::{
    calculate_utilization, detect_overallocations, level_resources, LevelingResult,
    OverallocationPeriod, ResourceTimeline, ResourceUtilization, ShiftedTask, UnresolvedConflict,
    UtilizationSummary,
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

    /// Analyze the effects of temporal constraints on a task
    fn analyze_constraint_effects(
        &self,
        project: &Project,
        task: &Task,
    ) -> Vec<utf8proj_core::ConstraintEffect> {
        use utf8proj_core::{ConstraintEffect, ConstraintEffectType, TaskConstraint};

        if task.constraints.is_empty() {
            return vec![];
        }

        // Schedule the project to get actual dates
        let schedule_result = Scheduler::schedule(self, project);
        let schedule = match schedule_result {
            Ok(s) => s,
            Err(_) => {
                // If scheduling fails, we can still describe the constraints
                return task
                    .constraints
                    .iter()
                    .map(|c| ConstraintEffect {
                        constraint: c.clone(),
                        effect: ConstraintEffectType::PushedStart, // Placeholder
                        description: format!(
                            "{} (scheduling failed - effect unknown)",
                            Self::format_constraint(c)
                        ),
                    })
                    .collect();
            }
        };

        // Get the scheduled task data
        let scheduled_task = match schedule.tasks.get(&task.id) {
            Some(t) => t,
            None => {
                return task
                    .constraints
                    .iter()
                    .map(|c| ConstraintEffect {
                        constraint: c.clone(),
                        effect: ConstraintEffectType::Redundant,
                        description: format!(
                            "{} (task not in schedule)",
                            Self::format_constraint(c)
                        ),
                    })
                    .collect();
            }
        };

        let es = scheduled_task.start;
        let ef = scheduled_task.finish;
        let ls = scheduled_task.late_start;
        let lf = scheduled_task.late_finish;
        let slack = scheduled_task.slack;
        let zero_slack = Duration::zero();

        task.constraints
            .iter()
            .map(|c| {
                let (effect, description) = match c {
                    TaskConstraint::MustStartOn(date) => {
                        if es == *date && ls == *date {
                            (
                                ConstraintEffectType::Pinned,
                                format!("Task pinned to start on {}", date),
                            )
                        } else if es == *date {
                            (
                                ConstraintEffectType::PushedStart,
                                format!("Constraint pushed early start to {}", date),
                            )
                        } else if es > *date {
                            (
                                ConstraintEffectType::Redundant,
                                format!(
                                    "Constraint date {} superseded by dependencies (ES={})",
                                    date, es
                                ),
                            )
                        } else {
                            (
                                ConstraintEffectType::CappedLate,
                                format!("Constraint capped late start at {}", date),
                            )
                        }
                    }
                    TaskConstraint::MustFinishOn(date) => {
                        if ef == *date && lf == *date {
                            (
                                ConstraintEffectType::Pinned,
                                format!("Task pinned to finish on {}", date),
                            )
                        } else if ef == *date {
                            (
                                ConstraintEffectType::PushedStart,
                                format!("Constraint pushed early finish to {}", date),
                            )
                        } else if ef > *date {
                            (
                                ConstraintEffectType::Redundant,
                                format!(
                                    "Constraint date {} superseded by dependencies (EF={})",
                                    date, ef
                                ),
                            )
                        } else {
                            (
                                ConstraintEffectType::CappedLate,
                                format!("Constraint capped late finish at {}", date),
                            )
                        }
                    }
                    TaskConstraint::StartNoEarlierThan(date) => {
                        if es == *date {
                            (
                                ConstraintEffectType::PushedStart,
                                format!("Task starts exactly on constraint boundary {}", date),
                            )
                        } else if es > *date {
                            (
                                ConstraintEffectType::Redundant,
                                format!(
                                    "Constraint {} redundant (dependencies already push ES to {})",
                                    date, es
                                ),
                            )
                        } else {
                            // es < date shouldn't happen if scheduling is correct
                            (
                                ConstraintEffectType::PushedStart,
                                format!("Constraint pushed early start to {}", date),
                            )
                        }
                    }
                    TaskConstraint::StartNoLaterThan(date) => {
                        if ls == *date {
                            if slack == zero_slack {
                                (
                                    ConstraintEffectType::CappedLate,
                                    format!(
                                        "Constraint made task critical (LS capped at {})",
                                        date
                                    ),
                                )
                            } else {
                                (
                                    ConstraintEffectType::CappedLate,
                                    format!("Constraint capped late start at {}", date),
                                )
                            }
                        } else if ls < *date {
                            (
                                ConstraintEffectType::Redundant,
                                format!(
                                    "Constraint {} redundant (successors already require LS={})",
                                    date, ls
                                ),
                            )
                        } else {
                            (
                                ConstraintEffectType::CappedLate,
                                format!("Constraint caps late start at {}", date),
                            )
                        }
                    }
                    TaskConstraint::FinishNoEarlierThan(date) => {
                        if ef == *date {
                            (
                                ConstraintEffectType::PushedStart,
                                format!("Task finishes exactly on constraint boundary {}", date),
                            )
                        } else if ef > *date {
                            (
                                ConstraintEffectType::Redundant,
                                format!(
                                    "Constraint {} redundant (dependencies already push EF to {})",
                                    date, ef
                                ),
                            )
                        } else {
                            (
                                ConstraintEffectType::PushedStart,
                                format!("Constraint pushed early finish to {}", date),
                            )
                        }
                    }
                    TaskConstraint::FinishNoLaterThan(date) => {
                        if lf == *date {
                            if slack == zero_slack {
                                (
                                    ConstraintEffectType::CappedLate,
                                    format!(
                                        "Constraint made task critical (LF capped at {})",
                                        date
                                    ),
                                )
                            } else {
                                (
                                    ConstraintEffectType::CappedLate,
                                    format!("Constraint capped late finish at {}", date),
                                )
                            }
                        } else if lf < *date {
                            (
                                ConstraintEffectType::Redundant,
                                format!(
                                    "Constraint {} redundant (successors already require LF={})",
                                    date, lf
                                ),
                            )
                        } else {
                            (
                                ConstraintEffectType::CappedLate,
                                format!("Constraint caps late finish at {}", date),
                            )
                        }
                    }
                };

                ConstraintEffect {
                    constraint: c.clone(),
                    effect,
                    description,
                }
            })
            .collect()
    }

    /// Format a constraint for display
    fn format_constraint(constraint: &utf8proj_core::TaskConstraint) -> String {
        use utf8proj_core::TaskConstraint;
        match constraint {
            TaskConstraint::MustStartOn(d) => format!("MustStartOn({})", d),
            TaskConstraint::MustFinishOn(d) => format!("MustFinishOn({})", d),
            TaskConstraint::StartNoEarlierThan(d) => format!("StartNoEarlierThan({})", d),
            TaskConstraint::StartNoLaterThan(d) => format!("StartNoLaterThan({})", d),
            TaskConstraint::FinishNoEarlierThan(d) => format!("FinishNoEarlierThan({})", d),
            TaskConstraint::FinishNoLaterThan(d) => format!("FinishNoLaterThan({})", d),
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

/// Extract task ID from an infeasible constraint error message
/// Expected format: "task 'task_id' has infeasible constraints: ..."
fn extract_task_from_infeasible_message(msg: &str) -> Option<String> {
    // Look for pattern: task 'xxx'
    if let Some(start) = msg.find("task '") {
        let rest = &msg[start + 6..];
        if let Some(end) = rest.find('\'') {
            return Some(rest[..end].to_string());
        }
    }
    None
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
// RFC-0001: Cost Calculation Helpers
// =============================================================================

/// Result of resolving a resource reference (could be resource or profile)
enum ResolvedAssignment<'a> {
    /// Concrete resource with fixed rate
    Concrete {
        rate: Option<&'a Money>,
        #[allow(dead_code)]
        resource_id: &'a str,
    },
    /// Abstract profile with rate range
    Abstract {
        rate_range: Option<RateRange>,
        #[allow(dead_code)]
        profile_id: &'a str,
    },
}

/// Resolve a resource_id to either a concrete Resource or abstract ResourceProfile
fn resolve_assignment<'a>(
    resource_id: &'a str,
    project: &'a Project,
) -> ResolvedAssignment<'a> {
    // First, check if it's a concrete resource
    if let Some(resource) = project.get_resource(resource_id) {
        return ResolvedAssignment::Concrete {
            rate: resource.rate.as_ref(),
            resource_id,
        };
    }

    // Otherwise, check if it's a profile
    if let Some(profile) = project.get_profile(resource_id) {
        let rate_range = resolve_profile_rate(profile, project);
        return ResolvedAssignment::Abstract {
            rate_range,
            profile_id: resource_id,
        };
    }

    // Unknown - treat as concrete with no rate
    ResolvedAssignment::Concrete {
        rate: None,
        resource_id,
    }
}

/// Resolve the effective rate range for a profile, applying trait multipliers
fn resolve_profile_rate(profile: &ResourceProfile, project: &Project) -> Option<RateRange> {
    // Get base rate from profile or inherited from parent
    let base_rate = get_profile_rate_range(profile, project)?;

    // Calculate combined trait multiplier (multiplicative composition)
    let trait_multiplier = calculate_trait_multiplier(&profile.traits, project);

    // Apply multiplier to the range
    Some(base_rate.apply_multiplier(trait_multiplier))
}

/// Get the rate range for a profile, walking up the specialization chain if needed
fn get_profile_rate_range(profile: &ResourceProfile, project: &Project) -> Option<RateRange> {
    // If this profile has a rate, use it
    if let Some(ref rate) = profile.rate {
        return match rate {
            utf8proj_core::ResourceRate::Range(range) => Some(range.clone()),
            utf8proj_core::ResourceRate::Fixed(money) => {
                // Convert fixed rate to collapsed range
                Some(RateRange::new(money.amount, money.amount))
            }
        };
    }

    // Otherwise, try to inherit from parent profile
    if let Some(ref parent_id) = profile.specializes {
        if let Some(parent) = project.get_profile(parent_id) {
            return get_profile_rate_range(parent, project);
        }
    }

    None
}

/// Calculate the combined trait multiplier (multiplicative)
fn calculate_trait_multiplier(trait_ids: &[String], project: &Project) -> f64 {
    let mut multiplier = 1.0;
    for trait_id in trait_ids {
        if let Some(t) = project.get_trait(trait_id) {
            multiplier *= t.rate_multiplier;
        }
    }
    multiplier
}

/// Calculate cost range for a single assignment
fn calculate_assignment_cost(
    resource_id: &str,
    units: f32,
    duration_days: i64,
    project: &Project,
) -> (Option<CostRange>, bool) {
    let resolved = resolve_assignment(resource_id, project);

    match resolved {
        ResolvedAssignment::Concrete { rate, .. } => {
            if let Some(money) = rate {
                // Fixed cost: rate × units × days
                let units_dec = Decimal::from_f32(units).unwrap_or(Decimal::ONE);
                let days_dec = Decimal::from(duration_days);
                let cost = money.amount * units_dec * days_dec;
                let cost_range = CostRange::fixed(cost, &money.currency);
                (Some(cost_range), false)
            } else {
                (None, false)
            }
        }
        ResolvedAssignment::Abstract { rate_range, .. } => {
            if let Some(range) = rate_range {
                // Cost range: (min, expected, max) × units × days
                let units_dec = Decimal::from_f32(units).unwrap_or(Decimal::ONE);
                let days_dec = Decimal::from(duration_days);
                let factor = units_dec * days_dec;
                let min_cost = range.min * factor;
                let max_cost = range.max * factor;
                let expected_cost = range.expected() * factor;
                let currency = range.currency.clone().unwrap_or_else(|| project.currency.clone());

                let cost_range = CostRange::new(min_cost, expected_cost, max_cost, currency);
                (Some(cost_range), true)
            } else {
                (None, true)
            }
        }
    }
}

/// Aggregate cost ranges from multiple assignments
fn aggregate_cost_ranges(ranges: &[CostRange]) -> Option<CostRange> {
    if ranges.is_empty() {
        return None;
    }

    let mut total = ranges[0].clone();
    for range in &ranges[1..] {
        total = total.add(range);
    }
    Some(total)
}

// =============================================================================
// Diagnostic Analysis
// =============================================================================

/// Configuration for diagnostic analysis
#[derive(Debug, Clone)]
pub struct AnalysisConfig {
    /// Source file path for diagnostic locations
    pub file: Option<PathBuf>,
    /// Cost spread threshold for W002 (percentage, default 50)
    pub cost_spread_threshold: f64,
}

impl Default for AnalysisConfig {
    fn default() -> Self {
        Self {
            file: None,
            cost_spread_threshold: 50.0,
        }
    }
}

impl AnalysisConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_file(mut self, file: impl Into<PathBuf>) -> Self {
        self.file = Some(file.into());
        self
    }

    pub fn with_cost_spread_threshold(mut self, threshold: f64) -> Self {
        self.cost_spread_threshold = threshold;
        self
    }
}

/// Analyze a project and emit diagnostics
///
/// This function performs semantic analysis on a project and emits
/// diagnostics for issues like abstract assignments, unused profiles, etc.
///
/// Call this after parsing but before or during scheduling.
pub fn analyze_project(
    project: &Project,
    schedule: Option<&Schedule>,
    config: &AnalysisConfig,
    emitter: &mut dyn DiagnosticEmitter,
) {
    // E001: Circular specialization
    check_circular_specialization(project, config, emitter);

    // W003: Unknown traits (check before E002 since it affects rate resolution)
    check_unknown_traits(project, config, emitter);

    // E002: Profile without rate (cost-bearing)
    check_profiles_without_rate(project, config, emitter);

    // Collect assignment info for task-level diagnostics
    let assignments_info = collect_assignment_info(project);

    // W001: Abstract assignments
    check_abstract_assignments(project, &assignments_info, config, emitter);

    // H001: Mixed abstraction level
    check_mixed_abstraction(project, &assignments_info, config, emitter);

    // W002: Wide cost range (requires schedule)
    if let Some(sched) = schedule {
        check_wide_cost_ranges(project, sched, config, emitter);
    }

    // H002: Unused profiles
    check_unused_profiles(project, &assignments_info, config, emitter);

    // H003: Unused traits
    check_unused_traits(project, config, emitter);

    // W005: Constraint zero slack (requires schedule)
    if let Some(sched) = schedule {
        check_constraint_zero_slack(project, sched, config, emitter);
    }

    // W006: Schedule variance (requires schedule)
    if let Some(sched) = schedule {
        check_schedule_variance(sched, config, emitter);
    }

    // I001: Project cost summary (requires schedule)
    if let Some(sched) = schedule {
        emit_project_summary(project, sched, &assignments_info, config, emitter);
    }

    // I004: Project status (requires schedule)
    if let Some(sched) = schedule {
        check_project_status(sched, config, emitter);
    }
}

/// Info about assignments in the project
struct AssignmentInfo {
    /// Map from resource/profile ID to list of (task_id, is_abstract)
    assignments: HashMap<String, Vec<(String, bool)>>,
    /// Set of profile IDs that are used in assignments
    used_profiles: std::collections::HashSet<String>,
    /// Set of trait IDs that are referenced by profiles
    used_traits: std::collections::HashSet<String>,
    /// Tasks with abstract assignments
    tasks_with_abstract: Vec<String>,
    /// Tasks with mixed (concrete + abstract) assignments
    tasks_with_mixed: Vec<String>,
}

fn collect_assignment_info(project: &Project) -> AssignmentInfo {
    let mut info = AssignmentInfo {
        assignments: HashMap::new(),
        used_profiles: std::collections::HashSet::new(),
        used_traits: std::collections::HashSet::new(),
        tasks_with_abstract: Vec::new(),
        tasks_with_mixed: Vec::new(),
    };

    // Collect all traits referenced by profiles
    for profile in &project.profiles {
        for trait_id in &profile.traits {
            info.used_traits.insert(trait_id.clone());
        }
    }

    // Collect assignments from all tasks (flattened)
    fn collect_from_tasks(
        tasks: &[Task],
        prefix: &str,
        project: &Project,
        info: &mut AssignmentInfo,
    ) {
        for task in tasks {
            let qualified_id = if prefix.is_empty() {
                task.id.clone()
            } else {
                format!("{}.{}", prefix, task.id)
            };

            let mut has_concrete = false;
            let mut has_abstract = false;

            for res_ref in &task.assigned {
                let is_abstract = project.get_profile(&res_ref.resource_id).is_some();

                if is_abstract {
                    has_abstract = true;
                    info.used_profiles.insert(res_ref.resource_id.clone());
                } else {
                    has_concrete = true;
                }

                info.assignments
                    .entry(res_ref.resource_id.clone())
                    .or_default()
                    .push((qualified_id.clone(), is_abstract));
            }

            if has_abstract {
                info.tasks_with_abstract.push(qualified_id.clone());
            }
            if has_concrete && has_abstract {
                info.tasks_with_mixed.push(qualified_id.clone());
            }

            // Recurse into children
            if !task.children.is_empty() {
                collect_from_tasks(&task.children, &qualified_id, project, info);
            }
        }
    }

    collect_from_tasks(&project.tasks, "", project, &mut info);
    info
}

/// E001: Check for circular specialization in profile inheritance
fn check_circular_specialization(
    project: &Project,
    config: &AnalysisConfig,
    emitter: &mut dyn DiagnosticEmitter,
) {
    for profile in &project.profiles {
        if let Some(cycle) = detect_specialization_cycle(profile, project) {
            let cycle_str = cycle.join(" -> ");
            emitter.emit(
                Diagnostic::error(
                    DiagnosticCode::E001CircularSpecialization,
                    format!("circular specialization detected: {}", cycle_str),
                )
                .with_file(config.file.clone().unwrap_or_default())
                .with_note(format!("cycle: {}", cycle_str))
                .with_hint("remove one specialization to break the cycle"),
            );
        }
    }
}

/// Detect if a profile's specialization chain contains a cycle
fn detect_specialization_cycle(profile: &ResourceProfile, project: &Project) -> Option<Vec<String>> {
    let mut visited = std::collections::HashSet::new();
    let mut path = Vec::new();
    let mut current = Some(profile);

    while let Some(p) = current {
        if visited.contains(&p.id) {
            // Found a cycle - extract the cycle portion
            let cycle_start = path.iter().position(|id| id == &p.id).unwrap();
            let mut cycle: Vec<String> = path[cycle_start..].to_vec();
            cycle.push(p.id.clone());
            return Some(cycle);
        }

        visited.insert(p.id.clone());
        path.push(p.id.clone());

        current = p.specializes.as_ref().and_then(|s| project.get_profile(s));
    }

    None
}

/// W003: Check for unknown trait references
fn check_unknown_traits(
    project: &Project,
    config: &AnalysisConfig,
    emitter: &mut dyn DiagnosticEmitter,
) {
    let defined_traits: std::collections::HashSet<_> =
        project.traits.iter().map(|t| t.id.as_str()).collect();

    for profile in &project.profiles {
        for trait_id in &profile.traits {
            if !defined_traits.contains(trait_id.as_str()) {
                emitter.emit(
                    Diagnostic::new(
                        DiagnosticCode::W003UnknownTrait,
                        format!(
                            "profile '{}' references unknown trait '{}'",
                            profile.id, trait_id
                        ),
                    )
                    .with_file(config.file.clone().unwrap_or_default())
                    .with_note("unknown traits are ignored (multiplier = 1.0)")
                    .with_hint("define the trait or remove the reference"),
                );
            }
        }
    }
}

/// E002: Check for profiles without rate that are used in assignments
fn check_profiles_without_rate(
    project: &Project,
    config: &AnalysisConfig,
    emitter: &mut dyn DiagnosticEmitter,
) {
    // First, collect which profiles are actually assigned to tasks
    let mut assigned_profiles: HashMap<String, Vec<String>> = HashMap::new();

    fn collect_assignments(tasks: &[Task], prefix: &str, map: &mut HashMap<String, Vec<String>>) {
        for task in tasks {
            let qualified_id = if prefix.is_empty() {
                task.id.clone()
            } else {
                format!("{}.{}", prefix, task.id)
            };

            for res_ref in &task.assigned {
                map.entry(res_ref.resource_id.clone())
                    .or_default()
                    .push(qualified_id.clone());
            }

            if !task.children.is_empty() {
                collect_assignments(&task.children, &qualified_id, map);
            }
        }
    }

    collect_assignments(&project.tasks, "", &mut assigned_profiles);

    // Check each profile
    for profile in &project.profiles {
        // Skip profiles that aren't assigned
        let tasks = match assigned_profiles.get(&profile.id) {
            Some(t) if !t.is_empty() => t,
            _ => continue,
        };

        // Check if profile has a rate (directly or inherited)
        let has_rate = get_profile_rate_range(profile, project).is_some();

        if !has_rate {
            let task_list = if tasks.len() <= 3 {
                tasks.join(", ")
            } else {
                format!("{}, ... ({} tasks)", tasks[..2].join(", "), tasks.len())
            };

            emitter.emit(
                Diagnostic::new(
                    DiagnosticCode::E002ProfileWithoutRate,
                    format!(
                        "profile '{}' has no rate defined but is assigned to tasks",
                        profile.id
                    ),
                )
                .with_file(config.file.clone().unwrap_or_default())
                .with_note(format!("cost calculations will be incomplete for: {}", task_list))
                .with_hint("add 'rate:' or 'rate_range:' block, or specialize from a profile with rate"),
            );
        }
    }
}

/// W001: Check for abstract assignments
fn check_abstract_assignments(
    project: &Project,
    info: &AssignmentInfo,
    config: &AnalysisConfig,
    emitter: &mut dyn DiagnosticEmitter,
) {
    // Flatten tasks to get task details
    let mut task_map: HashMap<String, &Task> = HashMap::new();
    flatten_tasks(&project.tasks, &mut task_map);

    for task_id in &info.tasks_with_abstract {
        if let Some(task) = task_map.get(task_id) {
            for res_ref in &task.assigned {
                if let Some(profile) = project.get_profile(&res_ref.resource_id) {
                    // Calculate the cost range for this assignment
                    let rate_range = resolve_profile_rate(profile, project);
                    let cost_note = if let Some(range) = rate_range {
                        let spread = range.spread_percent();
                        format!(
                            "cost range is ${} - ${} ({:.0}% spread)",
                            range.min, range.max, spread
                        )
                    } else {
                        "cost range is unknown (no rate defined)".to_string()
                    };

                    emitter.emit(
                        Diagnostic::new(
                            DiagnosticCode::W001AbstractAssignment,
                            format!(
                                "task '{}' is assigned to abstract profile '{}'",
                                task_id, res_ref.resource_id
                            ),
                        )
                        .with_file(config.file.clone().unwrap_or_default())
                        .with_note(cost_note)
                        .with_hint("assign a concrete resource to lock in exact cost"),
                    );
                }
            }
        }
    }
}

/// H001: Check for mixed abstraction level in assignments
fn check_mixed_abstraction(
    project: &Project,
    info: &AssignmentInfo,
    config: &AnalysisConfig,
    emitter: &mut dyn DiagnosticEmitter,
) {
    let mut task_map: HashMap<String, &Task> = HashMap::new();
    flatten_tasks(&project.tasks, &mut task_map);

    for task_id in &info.tasks_with_mixed {
        if let Some(task) = task_map.get(task_id) {
            let _concrete: Vec<_> = task
                .assigned
                .iter()
                .filter(|r| project.get_profile(&r.resource_id).is_none())
                .map(|r| r.resource_id.as_str())
                .collect();
            let abstract_: Vec<_> = task
                .assigned
                .iter()
                .filter(|r| project.get_profile(&r.resource_id).is_some())
                .map(|r| r.resource_id.as_str())
                .collect();

            if !abstract_.is_empty() {
                emitter.emit(
                    Diagnostic::new(
                        DiagnosticCode::H001MixedAbstraction,
                        format!("task '{}' mixes concrete and abstract assignments", task_id),
                    )
                    .with_file(config.file.clone().unwrap_or_default())
                    .with_note("this is valid but may indicate incomplete refinement")
                    .with_hint(format!(
                        "consider refining '{}' to a concrete resource",
                        abstract_.join("', '")
                    )),
                );
            }
        }
    }
}

/// W002: Check for wide cost ranges
fn check_wide_cost_ranges(
    project: &Project,
    schedule: &Schedule,
    config: &AnalysisConfig,
    emitter: &mut dyn DiagnosticEmitter,
) {
    for (task_id, scheduled_task) in &schedule.tasks {
        if let Some(ref cost_range) = scheduled_task.cost_range {
            let spread = cost_range.spread_percent();
            if spread > config.cost_spread_threshold {
                // Find contributing factors
                let mut contributors: Vec<String> = Vec::new();

                // Get task details
                let mut task_map: HashMap<String, &Task> = HashMap::new();
                flatten_tasks(&project.tasks, &mut task_map);

                if let Some(task) = task_map.get(task_id) {
                    for res_ref in &task.assigned {
                        if let Some(profile) = project.get_profile(&res_ref.resource_id) {
                            if let Some(rate) = resolve_profile_rate(profile, project) {
                                contributors.push(format!(
                                    "{}: ${} - ${}/day",
                                    res_ref.resource_id, rate.min, rate.max
                                ));
                            }
                            for trait_id in &profile.traits {
                                if let Some(t) = project.get_trait(trait_id) {
                                    if (t.rate_multiplier - 1.0).abs() > 0.01 {
                                        contributors.push(format!(
                                            "{} trait: {}x multiplier",
                                            trait_id, t.rate_multiplier
                                        ));
                                    }
                                }
                            }
                        }
                    }
                }

                let mut diag = Diagnostic::new(
                    DiagnosticCode::W002WideCostRange,
                    format!(
                        "task '{}' has wide cost uncertainty ({:.0}% spread)",
                        task_id, spread
                    ),
                )
                .with_file(config.file.clone().unwrap_or_default())
                .with_note(format!(
                    "cost range: ${} - ${} (expected: ${})",
                    cost_range.min, cost_range.max, cost_range.expected
                ));

                if !contributors.is_empty() {
                    diag = diag.with_note(format!("contributors: {}", contributors.join(", ")));
                }

                diag = diag.with_hint("narrow the profile rate range or assign concrete resources");

                emitter.emit(diag);
            }
        }
    }
}

/// H002: Check for unused profiles
fn check_unused_profiles(
    project: &Project,
    info: &AssignmentInfo,
    config: &AnalysisConfig,
    emitter: &mut dyn DiagnosticEmitter,
) {
    for profile in &project.profiles {
        if !info.used_profiles.contains(&profile.id) {
            emitter.emit(
                Diagnostic::new(
                    DiagnosticCode::H002UnusedProfile,
                    format!("profile '{}' is defined but never assigned", profile.id),
                )
                .with_file(config.file.clone().unwrap_or_default())
                .with_hint("assign to tasks or remove if no longer needed"),
            );
        }
    }
}

/// H003: Check for unused traits
fn check_unused_traits(
    project: &Project,
    config: &AnalysisConfig,
    emitter: &mut dyn DiagnosticEmitter,
) {
    // Collect all traits referenced by any profile
    let mut used: std::collections::HashSet<String> = std::collections::HashSet::new();
    for profile in &project.profiles {
        for trait_id in &profile.traits {
            used.insert(trait_id.clone());
        }
    }

    for t in &project.traits {
        if !used.contains(&t.id) {
            emitter.emit(
                Diagnostic::new(
                    DiagnosticCode::H003UnusedTrait,
                    format!("trait '{}' is defined but never referenced", t.id),
                )
                .with_file(config.file.clone().unwrap_or_default())
                .with_hint("add to profile traits or remove if no longer needed"),
            );
        }
    }
}

/// W005: Check for tasks where constraints reduced slack to zero
fn check_constraint_zero_slack(
    project: &Project,
    schedule: &Schedule,
    config: &AnalysisConfig,
    emitter: &mut dyn DiagnosticEmitter,
) {
    // Build task map for constraint lookup
    let mut task_map: HashMap<String, &Task> = HashMap::new();
    flatten_tasks(&project.tasks, &mut task_map);

    for (task_id, scheduled) in &schedule.tasks {
        // Only check tasks with zero slack
        if scheduled.slack != Duration::zero() {
            continue;
        }

        // Get the original task to check constraints
        let Some(task) = task_map.get(task_id) else {
            continue;
        };

        // Check for ceiling constraints that could cause zero slack
        // These are: MustStartOn, MustFinishOn, StartNoLaterThan, FinishNoLaterThan
        let has_ceiling_constraint = task.constraints.iter().any(|c| {
            matches!(
                c,
                TaskConstraint::MustStartOn(_)
                    | TaskConstraint::MustFinishOn(_)
                    | TaskConstraint::StartNoLaterThan(_)
                    | TaskConstraint::FinishNoLaterThan(_)
            )
        });

        if has_ceiling_constraint {
            // Find the specific constraint for the message
            let constraint_desc = task
                .constraints
                .iter()
                .find_map(|c| match c {
                    TaskConstraint::MustStartOn(d) => Some(format!("must_start_on: {}", d)),
                    TaskConstraint::MustFinishOn(d) => Some(format!("must_finish_on: {}", d)),
                    TaskConstraint::StartNoLaterThan(d) => {
                        Some(format!("start_no_later_than: {}", d))
                    }
                    TaskConstraint::FinishNoLaterThan(d) => {
                        Some(format!("finish_no_later_than: {}", d))
                    }
                    _ => None,
                })
                .unwrap_or_else(|| "constraint".to_string());

            emitter.emit(
                Diagnostic::new(
                    DiagnosticCode::W005ConstraintZeroSlack,
                    format!("constraint reduces slack to zero for task '{}'", task_id),
                )
                .with_file(config.file.clone().unwrap_or_default())
                .with_note(format!("{} makes task critical", constraint_desc))
                .with_hint("consider relaxing constraint or adding buffer"),
            );
        }
    }
}

/// Default threshold for variance warnings (days)
const VARIANCE_THRESHOLD_DAYS: i64 = 5;

/// W006: Check for tasks with significant schedule variance
fn check_schedule_variance(
    schedule: &Schedule,
    config: &AnalysisConfig,
    emitter: &mut dyn DiagnosticEmitter,
) {
    for (task_id, scheduled) in &schedule.tasks {
        // Only warn if finish variance exceeds threshold
        if scheduled.finish_variance_days > VARIANCE_THRESHOLD_DAYS {
            let variance_str = format!("+{}d", scheduled.finish_variance_days);
            emitter.emit(
                Diagnostic::new(
                    DiagnosticCode::W006ScheduleVariance,
                    format!("task '{}' is slipping ({})", task_id, variance_str),
                )
                .with_file(config.file.clone().unwrap_or_default())
                .with_note(format!(
                    "baseline finish: {}, forecast finish: {}",
                    scheduled.baseline_finish, scheduled.forecast_finish
                ))
                .with_hint("review progress or adjust plan"),
            );
        }
    }
}

/// I004: Emit project status (overall progress and variance)
fn check_project_status(
    schedule: &Schedule,
    config: &AnalysisConfig,
    emitter: &mut dyn DiagnosticEmitter,
) {
    let variance_indicator = if schedule.project_variance_days > 0 {
        format!("+{}d behind", schedule.project_variance_days)
    } else if schedule.project_variance_days < 0 {
        format!("{}d ahead", schedule.project_variance_days.abs())
    } else {
        "on schedule".to_string()
    };

    let status_emoji = if schedule.project_variance_days > VARIANCE_THRESHOLD_DAYS {
        "🔴"
    } else if schedule.project_variance_days > 0 {
        "🟡"
    } else {
        "🟢"
    };

    emitter.emit(
        Diagnostic::new(
            DiagnosticCode::I004ProjectStatus,
            format!(
                "project {}% complete, {} {}",
                schedule.project_progress, variance_indicator, status_emoji
            ),
        )
        .with_file(config.file.clone().unwrap_or_default())
        .with_note(format!(
            "baseline finish: {}, forecast finish: {}",
            schedule.project_baseline_finish, schedule.project_forecast_finish
        )),
    );
}

/// I001: Emit project cost summary
fn emit_project_summary(
    project: &Project,
    schedule: &Schedule,
    _info: &AssignmentInfo,
    _config: &AnalysisConfig,
    emitter: &mut dyn DiagnosticEmitter,
) {
    let concrete_count = schedule
        .tasks
        .values()
        .filter(|t| !t.has_abstract_assignments && !t.assignments.is_empty())
        .count();
    let abstract_count = schedule
        .tasks
        .values()
        .filter(|t| t.has_abstract_assignments)
        .count();

    let cost_str = if let Some(ref cost) = schedule.total_cost_range {
        if cost.is_fixed() {
            format!("${}", cost.expected)
        } else {
            format!(
                "${} - ${} (expected: ${})",
                cost.min, cost.max, cost.expected
            )
        }
    } else {
        "unknown (no cost data)".to_string()
    };

    emitter.emit(
        Diagnostic::new(
            DiagnosticCode::I001ProjectCostSummary,
            format!("project '{}' scheduled successfully", project.name),
        )
        .with_note(format!(
            "duration: {} days ({} to {})",
            schedule.project_duration.as_days() as i64,
            project.start,
            schedule.project_end
        ))
        .with_note(format!("cost: {}", cost_str))
        .with_note(format!(
            "tasks: {} ({} concrete, {} abstract assignments)",
            schedule.tasks.len(),
            concrete_count,
            abstract_count
        ))
        .with_note(format!("critical path: {} tasks", schedule.critical_path.len())),
    );
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
                total_cost_range: None,
                project_progress: 0,
                project_baseline_finish: project.start,
                project_forecast_finish: project.start,
                project_variance_days: 0,
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

                // Apply floor constraints to ES (forward pass)
                // These constrain how early the task can start/finish
                // Note: internal early_finish is exclusive (first day after task)
                let mut min_finish: Option<i64> = None;
                for constraint in &task.constraints {
                    match constraint {
                        TaskConstraint::MustStartOn(date) | TaskConstraint::StartNoEarlierThan(date) => {
                            // ES ≥ constraint date
                            let constraint_days = date_to_working_days(project.start, *date, &calendar);
                            es = es.max(constraint_days);
                        }
                        TaskConstraint::MustFinishOn(date) | TaskConstraint::FinishNoEarlierThan(date) => {
                            // EF ≥ constraint date (inclusive)
                            // Internal EF is exclusive, so add 1: if task must finish ON day 9,
                            // internal EF must be 10 (first day after)
                            let constraint_days = date_to_working_days(project.start, *date, &calendar);
                            let exclusive_ef = constraint_days + 1;
                            min_finish = Some(min_finish.map_or(exclusive_ef, |mf| mf.max(exclusive_ef)));
                        }
                        _ => {} // Ceiling constraints handled in backward pass
                    }
                }

                // EF = ES + duration (initial calculation)
                let mut ef = es + duration;

                // If finish constraint pushes EF forward, shift ES accordingly
                if let Some(mf) = min_finish {
                    if mf > ef {
                        ef = mf;
                        es = ef - duration;
                    }
                }

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

            // LF = min(constraint from each successor), or project_end if no successors
            // The constraint depends on the dependency type:
            //   FS: LF(pred) <= LS(succ) - lag
            //   SS: LF(pred) <= LS(succ) - lag + duration(pred)
            //   FF: LF(pred) <= LF(succ) - lag
            //   SF: LF(pred) <= LF(succ) - lag + duration(pred)
            let lf = match successors {
                Some(succs) if !succs.is_empty() => {
                    let mut min_lf = project_end_days;
                    for succ_id in succs {
                        if let Some(succ_node) = nodes.get(succ_id) {
                            // Find the dependency type from successor's depends list
                            let succ_task = task_map.get(succ_id);
                            let dep_info = succ_task.and_then(|t| {
                                t.depends.iter().find(|d| {
                                    // Check if this dependency refers to current task
                                    let resolved = resolve_dependency_path(
                                        &d.predecessor,
                                        succ_id,
                                        &context_map,
                                        &task_map,
                                    );
                                    resolved.as_ref() == Some(id)
                                })
                            });

                            let constraint_lf = if let Some(dep) = dep_info {
                                let lag = dep.lag.map(|d| d.as_days() as i64).unwrap_or(0);
                                match dep.dep_type {
                                    DependencyType::FinishToStart => {
                                        // LF(pred) <= LS(succ) - lag
                                        // For negative lag, mirror the forward pass formula:
                                        // Forward uses pred.EF - 1 + lag, so backward uses LS + 1 - lag
                                        if lag >= 0 {
                                            succ_node.late_start - lag
                                        } else {
                                            succ_node.late_start + 1 - lag
                                        }
                                    }
                                    DependencyType::StartToStart => {
                                        // LS(pred) <= LS(succ) - lag
                                        // LF(pred) = LS(pred) + duration = LS(succ) - lag + duration
                                        succ_node.late_start - lag + duration
                                    }
                                    DependencyType::FinishToFinish => {
                                        // LF(pred) <= LF(succ) - lag
                                        succ_node.late_finish - lag
                                    }
                                    DependencyType::StartToFinish => {
                                        // LS(pred) <= LF(succ) - lag
                                        // LF(pred) = LF(succ) - lag + duration
                                        succ_node.late_finish - lag + duration
                                    }
                                }
                            } else {
                                // Default to FS behavior
                                succ_node.late_start
                            };
                            min_lf = min_lf.min(constraint_lf);
                        }
                    }
                    min_lf
                }
                _ => project_end_days,
            };

            // Apply ceiling constraints to LF/LS (backward pass)
            // Note: internal late_finish is exclusive (first day after task)
            let task = task_map.get(id);
            let mut max_finish: Option<i64> = None;
            let mut max_start: Option<i64> = None;
            if let Some(task) = task {
                for constraint in &task.constraints {
                    match constraint {
                        TaskConstraint::MustFinishOn(date) | TaskConstraint::FinishNoLaterThan(date) => {
                            // LF ≤ constraint date (inclusive)
                            // Internal LF is exclusive, so add 1
                            let constraint_days = date_to_working_days(project.start, *date, &calendar);
                            let exclusive_lf = constraint_days + 1;
                            max_finish = Some(max_finish.map_or(exclusive_lf, |mf| mf.min(exclusive_lf)));
                        }
                        TaskConstraint::MustStartOn(date) | TaskConstraint::StartNoLaterThan(date) => {
                            // LS ≤ constraint date
                            let constraint_days = date_to_working_days(project.start, *date, &calendar);
                            max_start = Some(max_start.map_or(constraint_days, |ms| ms.min(constraint_days)));
                        }
                        _ => {} // Floor constraints already handled in forward pass
                    }
                }
            }

            // Apply finish ceiling if specified
            let lf = if let Some(mf) = max_finish {
                lf.min(mf)
            } else {
                lf
            };

            // LS = LF - duration (initial calculation)
            let mut ls = lf - duration;

            // Apply start ceiling if specified
            if let Some(ms) = max_start {
                ls = ls.min(ms);
            }

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

        // Step 7c: Feasibility check - verify ES <= LS for all tasks
        // If any task has negative slack, constraints are infeasible
        // Sort by task ID for deterministic error reporting
        let mut infeasibility_check_ids: Vec<_> = nodes.keys().collect();
        infeasibility_check_ids.sort();
        for id in infeasibility_check_ids {
            let node = &nodes[id];
            if node.slack < 0 {
                return Err(ScheduleError::Infeasible(format!(
                    "task '{}' has infeasible constraints: ES ({}) > LS ({}), slack = {} days",
                    id, node.early_start, node.late_start, node.slack
                )));
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

            // Build assignments with RFC-0001 cost calculation
            let mut assignments: Vec<Assignment> = Vec::new();
            let mut task_cost_ranges: Vec<CostRange> = Vec::new();
            let mut has_abstract = false;

            for res_ref in &node.task.assigned {
                let (cost_range, is_abstract) = calculate_assignment_cost(
                    &res_ref.resource_id,
                    res_ref.units,
                    node.duration_days,
                    project,
                );

                if is_abstract {
                    has_abstract = true;
                }

                // Track cost ranges for aggregation
                if let Some(ref range) = cost_range {
                    task_cost_ranges.push(range.clone());
                }

                // For concrete assignments, extract fixed cost
                let fixed_cost = if !is_abstract {
                    cost_range.as_ref().map(|r| Money::new(r.expected, &r.currency))
                } else {
                    None
                };

                assignments.push(Assignment {
                    resource_id: res_ref.resource_id.clone(),
                    start: start_date,
                    finish: finish_date,
                    units: res_ref.units,
                    cost: fixed_cost,
                    cost_range: cost_range.clone(),
                    is_abstract,
                });
            }

            // Aggregate task-level cost range
            let task_cost_range = aggregate_cost_ranges(&task_cost_ranges);

            // Progress tracking calculations
            let task = node.task;
            // Use effective_progress() to get derived progress for containers
            let percent_complete = task.effective_progress();
            let status = task.derived_status();

            // Calculate remaining_duration using SCHEDULED duration (not raw effort)
            // This accounts for resource parallelism
            let scheduled_duration_days = node.duration_days as f64;
            let remaining = if percent_complete == 100 {
                Duration::zero()
            } else {
                let remaining_pct = 1.0 - (percent_complete as f64 / 100.0);
                Duration::days((scheduled_duration_days * remaining_pct).ceil() as i64)
            };

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

            // Variance calculation (calendar days, positive = late)
            let start_variance_days = (forecast_start - start_date).num_days();
            let finish_variance_days = (forecast_finish - finish_date).num_days();

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
                    // Variance fields (baseline vs forecast)
                    baseline_start: start_date,
                    baseline_finish: finish_date,
                    start_variance_days,
                    finish_variance_days,
                    // RFC-0001: Cost range fields
                    cost_range: task_cost_range,
                    has_abstract_assignments: has_abstract,
                },
            );
        }

        // Aggregate project-level cost ranges from all tasks
        let all_task_cost_ranges: Vec<CostRange> = scheduled_tasks
            .values()
            .filter_map(|st| st.cost_range.clone())
            .collect();
        let total_cost_range = aggregate_cost_ranges(&all_task_cost_ranges);

        // Step 10: Build final schedule
        // project_end is the last working day of the project
        let project_end_date = if project_end_days > 0 {
            working_day_cache.get(project_end_days - 1)
        } else {
            project.start
        };

        // Step 10b: Compute project-level progress and variance (I004)
        // Progress: weighted average of leaf task progress, weighted by duration
        // Variance: max(forecast_finish) - max(baseline_finish)
        let (project_progress, project_baseline_finish, project_forecast_finish) = {
            let mut total_weight: i64 = 0;
            let mut weighted_progress: i64 = 0;
            let mut max_baseline = project.start;
            let mut max_forecast = project.start;

            // Build set of container task IDs (tasks with children)
            let container_ids: std::collections::HashSet<&str> = task_map
                .values()
                .filter(|t| !t.children.is_empty())
                .map(|t| t.id.as_str())
                .collect();

            for st in scheduled_tasks.values() {
                // Update baseline/forecast max for ALL tasks
                if st.baseline_finish > max_baseline {
                    max_baseline = st.baseline_finish;
                }
                if st.forecast_finish > max_forecast {
                    max_forecast = st.forecast_finish;
                }

                // Only aggregate progress from leaf tasks (non-containers)
                if !container_ids.contains(st.task_id.as_str()) {
                    let duration_days = st.duration.as_days() as i64;
                    if duration_days > 0 {
                        total_weight += duration_days;
                        weighted_progress += (st.percent_complete as i64) * duration_days;
                    }
                }
            }

            let progress = if total_weight > 0 {
                (weighted_progress / total_weight) as u8
            } else {
                0
            };

            (progress, max_baseline, max_forecast)
        };

        let project_variance_days =
            (project_forecast_finish - project_baseline_finish).num_days();

        let schedule = Schedule {
            tasks: scheduled_tasks,
            critical_path,
            project_duration: Duration::days(project_end_days),
            project_end: project_end_date,
            total_cost: None, // For fully concrete projects
            total_cost_range, // RFC-0001: Cost range for abstract assignments
            project_progress,
            project_baseline_finish,
            project_forecast_finish,
            project_variance_days,
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
        use utf8proj_core::{Conflict, ConflictType, ScheduleError, Suggestion};

        // Step 1: Check for circular dependencies
        let (task_map, context_map) = flatten_tasks_with_context(&project.tasks);

        if let Err(e) = topological_sort(&task_map, &context_map) {
            return FeasibilityResult {
                feasible: false,
                conflicts: vec![Conflict {
                    conflict_type: ConflictType::CircularDependency,
                    description: e.to_string(),
                    involved_tasks: vec![],
                    involved_resources: vec![],
                }],
                suggestions: vec![],
            };
        }

        // Step 2: Try to schedule and check for constraint conflicts
        match Scheduler::schedule(self, project) {
            Ok(_schedule) => FeasibilityResult {
                feasible: true,
                conflicts: vec![],
                suggestions: vec![],
            },
            Err(e) => {
                let (conflict_type, involved_tasks, suggestions) = match &e {
                    ScheduleError::Infeasible(msg) => {
                        // Extract task ID from the error message if possible
                        let task_id = extract_task_from_infeasible_message(msg);
                        let suggestions = if let Some(ref id) = task_id {
                            vec![Suggestion {
                                description: format!(
                                    "Review constraints on task '{}' or relax conflicting dependencies",
                                    id
                                ),
                                impact: "May allow schedule to be computed".to_string(),
                            }]
                        } else {
                            vec![]
                        };
                        (
                            ConflictType::ImpossibleConstraint,
                            task_id.into_iter().collect(),
                            suggestions,
                        )
                    }
                    ScheduleError::CircularDependency(_) => (
                        ConflictType::CircularDependency,
                        vec![],
                        vec![],
                    ),
                    ScheduleError::TaskNotFound(id) => (
                        ConflictType::ImpossibleConstraint,
                        vec![id.clone()],
                        vec![],
                    ),
                    _ => (ConflictType::ImpossibleConstraint, vec![], vec![]),
                };

                FeasibilityResult {
                    feasible: false,
                    conflicts: vec![Conflict {
                        conflict_type,
                        description: e.to_string(),
                        involved_tasks,
                        involved_resources: vec![],
                    }],
                    suggestions,
                }
            }
        }
    }

    fn explain(&self, project: &Project, task_id: &TaskId) -> Explanation {

        // Try to find the task and explain its scheduling
        let mut task_map: HashMap<String, &Task> = HashMap::new();
        flatten_tasks(&project.tasks, &mut task_map);

        if let Some(task) = task_map.get(task_id) {
            let dependency_constraints: Vec<String> = task
                .depends
                .iter()
                .map(|d| format!("Depends on: {}", d.predecessor))
                .collect();

            // Build constraint effects from temporal constraints
            let constraint_effects = self.analyze_constraint_effects(project, task);

            Explanation {
                task_id: task_id.clone(),
                reason: if task.depends.is_empty() && task.constraints.is_empty() {
                    "Scheduled at project start (no dependencies or constraints)".into()
                } else if task.depends.is_empty() {
                    "Scheduled based on constraints".into()
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
                constraints_applied: dependency_constraints,
                alternatives_considered: vec![],
                constraint_effects,
            }
        } else {
            Explanation {
                task_id: task_id.clone(),
                reason: "Task not found".into(),
                constraints_applied: vec![],
                alternatives_considered: vec![],
                constraint_effects: vec![],
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
    fn feasibility_check_constraint_conflict() {
        use utf8proj_core::{ConflictType, Scheduler, TaskConstraint};

        let mut project = Project::new("Constraint Conflict");
        project.start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        project.tasks = vec![
            Task::new("blocker").effort(Duration::days(10)),
            Task::new("blocked")
                .effort(Duration::days(5))
                .depends_on("blocker")
                // This constraint conflicts with the dependency -
                // blocker finishes Jan 17, but constraint says start Jan 10
                .constraint(TaskConstraint::MustStartOn(
                    NaiveDate::from_ymd_opt(2025, 1, 10).unwrap(),
                )),
        ];

        let solver = CpmSolver::new();
        let result = solver.is_feasible(&project);

        assert!(!result.feasible);
        assert!(!result.conflicts.is_empty());
        assert_eq!(
            result.conflicts[0].conflict_type,
            ConflictType::ImpossibleConstraint
        );
        // Should have the involved task
        assert!(!result.conflicts[0].involved_tasks.is_empty());
        // Should have suggestions
        assert!(!result.suggestions.is_empty());
    }

    #[test]
    fn feasibility_check_valid_constraints() {
        use utf8proj_core::{Scheduler, TaskConstraint};

        let mut project = Project::new("Valid Constraints");
        project.start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        project.tasks = vec![
            Task::new("task1")
                .effort(Duration::days(5))
                // Constraint is at project start - no conflict
                .constraint(TaskConstraint::StartNoEarlierThan(
                    NaiveDate::from_ymd_opt(2025, 1, 6).unwrap(),
                )),
        ];

        let solver = CpmSolver::new();
        let result = solver.is_feasible(&project);

        assert!(result.feasible);
        assert!(result.conflicts.is_empty());
    }

    #[test]
    fn extract_task_from_infeasible_message_works() {
        let msg = "task 'my_task' has infeasible constraints: ES (10) > LS (4), slack = -6 days";
        let result = super::extract_task_from_infeasible_message(msg);
        assert_eq!(result, Some("my_task".to_string()));

        let msg2 = "task 'nested.task.id' has infeasible constraints";
        let result2 = super::extract_task_from_infeasible_message(msg2);
        assert_eq!(result2, Some("nested.task.id".to_string()));

        let msg3 = "some other error message";
        let result3 = super::extract_task_from_infeasible_message(msg3);
        assert_eq!(result3, None);
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
    fn explain_task_with_dependencies_shows_constraints() {
        // Verify that explain() populates constraints_applied for tasks with dependencies
        let project = make_test_project();
        let solver = CpmSolver::new();

        // "implement" depends on "design"
        let explanation = solver.explain(&project, &"implement".to_string());

        assert_eq!(explanation.task_id, "implement");
        assert!(explanation.reason.contains("predecessors"));
        assert!(!explanation.constraints_applied.is_empty());
        assert!(explanation.constraints_applied.iter().any(|c| c.contains("design")));
    }

    #[test]
    fn explain_task_with_temporal_constraint_shows_effects() {
        use utf8proj_core::{ConstraintEffectType, TaskConstraint};

        let mut project = Project::new("Constraint Test");
        project.start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        project.tasks = vec![
            Task::new("constrained")
                .effort(Duration::days(5))
                .constraint(TaskConstraint::StartNoEarlierThan(
                    NaiveDate::from_ymd_opt(2025, 1, 13).unwrap(),
                )),
        ];

        let solver = CpmSolver::new();
        let explanation = solver.explain(&project, &"constrained".to_string());

        assert_eq!(explanation.task_id, "constrained");
        assert_eq!(explanation.constraint_effects.len(), 1);

        let effect = &explanation.constraint_effects[0];
        assert!(matches!(
            effect.constraint,
            TaskConstraint::StartNoEarlierThan(_)
        ));
        assert_eq!(effect.effect, ConstraintEffectType::PushedStart);
        assert!(effect.description.contains("2025-01-13"));
    }

    #[test]
    fn explain_task_with_pinned_constraint() {
        use utf8proj_core::{ConstraintEffectType, TaskConstraint};

        let mut project = Project::new("Pin Test");
        project.start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        project.tasks = vec![
            Task::new("pinned")
                .effort(Duration::days(3))
                .constraint(TaskConstraint::MustStartOn(
                    NaiveDate::from_ymd_opt(2025, 1, 6).unwrap(),
                )),
        ];

        let solver = CpmSolver::new();
        let explanation = solver.explain(&project, &"pinned".to_string());

        assert_eq!(explanation.constraint_effects.len(), 1);
        let effect = &explanation.constraint_effects[0];
        assert_eq!(effect.effect, ConstraintEffectType::Pinned);
        assert!(effect.description.contains("pinned"));
    }

    #[test]
    fn explain_task_with_redundant_constraint() {
        use utf8proj_core::{ConstraintEffectType, TaskConstraint};

        let mut project = Project::new("Redundant Test");
        project.start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        project.tasks = vec![
            Task::new("blocker").effort(Duration::days(10)),
            Task::new("blocked")
                .effort(Duration::days(5))
                .depends_on("blocker")
                // SNET is before where dependencies push it - should be redundant
                .constraint(TaskConstraint::StartNoEarlierThan(
                    NaiveDate::from_ymd_opt(2025, 1, 10).unwrap(),
                )),
        ];

        let solver = CpmSolver::new();
        let explanation = solver.explain(&project, &"blocked".to_string());

        assert_eq!(explanation.constraint_effects.len(), 1);
        let effect = &explanation.constraint_effects[0];
        assert_eq!(effect.effect, ConstraintEffectType::Redundant);
        assert!(effect.description.contains("redundant"));
    }

    #[test]
    fn explain_task_without_constraints_has_empty_effects() {
        let project = make_test_project();
        let solver = CpmSolver::new();

        let explanation = solver.explain(&project, &"design".to_string());

        assert!(explanation.constraint_effects.is_empty());
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

    // =============================================================================
    // RFC-0001: Progressive Resource Refinement Tests
    // =============================================================================

    #[test]
    fn trait_multiplier_single() {
        use utf8proj_core::Trait;

        let mut project = Project::new("Test");
        project.traits.push(Trait::new("senior").rate_multiplier(1.3));

        let multiplier = calculate_trait_multiplier(&["senior".to_string()], &project);
        assert!((multiplier - 1.3).abs() < 0.001);
    }

    #[test]
    fn trait_multiplier_multiplicative() {
        use utf8proj_core::Trait;

        let mut project = Project::new("Test");
        project.traits.push(Trait::new("senior").rate_multiplier(1.3));
        project.traits.push(Trait::new("contractor").rate_multiplier(1.2));

        let multiplier = calculate_trait_multiplier(
            &["senior".to_string(), "contractor".to_string()],
            &project,
        );
        // 1.3 × 1.2 = 1.56
        assert!((multiplier - 1.56).abs() < 0.001);
    }

    #[test]
    fn trait_multiplier_unknown_trait_ignored() {
        use utf8proj_core::Trait;

        let mut project = Project::new("Test");
        project.traits.push(Trait::new("senior").rate_multiplier(1.3));

        let multiplier = calculate_trait_multiplier(
            &["senior".to_string(), "unknown".to_string()],
            &project,
        );
        // Unknown trait has no effect (multiplied by 1.0 implicitly)
        assert!((multiplier - 1.3).abs() < 0.001);
    }

    #[test]
    fn resolve_profile_rate_basic() {
        let mut project = Project::new("Test");
        project.profiles.push(
            ResourceProfile::new("developer")
                .rate_range(RateRange::new(Decimal::from(50), Decimal::from(100))),
        );

        let profile = project.get_profile("developer").unwrap();
        let rate = resolve_profile_rate(profile, &project).unwrap();

        assert_eq!(rate.min, Decimal::from(50));
        assert_eq!(rate.max, Decimal::from(100));
    }

    #[test]
    fn resolve_profile_rate_with_trait_multiplier() {
        use utf8proj_core::Trait;

        let mut project = Project::new("Test");
        project.traits.push(Trait::new("senior").rate_multiplier(1.5));
        project.profiles.push(
            ResourceProfile::new("developer")
                .rate_range(RateRange::new(Decimal::from(100), Decimal::from(200)))
                .with_trait("senior"),
        );

        let profile = project.get_profile("developer").unwrap();
        let rate = resolve_profile_rate(profile, &project).unwrap();

        // 100 × 1.5 = 150, 200 × 1.5 = 300
        assert_eq!(rate.min, Decimal::from(150));
        assert_eq!(rate.max, Decimal::from(300));
    }

    #[test]
    fn resolve_profile_rate_inherited() {
        let mut project = Project::new("Test");
        project.profiles.push(
            ResourceProfile::new("developer")
                .rate_range(RateRange::new(Decimal::from(80), Decimal::from(120))),
        );
        project.profiles.push(
            ResourceProfile::new("senior_developer")
                .specializes("developer"),
        );

        let profile = project.get_profile("senior_developer").unwrap();
        let rate = resolve_profile_rate(profile, &project).unwrap();

        // Inherits rate from parent "developer"
        assert_eq!(rate.min, Decimal::from(80));
        assert_eq!(rate.max, Decimal::from(120));
    }

    #[test]
    fn resolve_assignment_concrete_resource() {
        use utf8proj_core::Resource;

        let mut project = Project::new("Test");
        project.resources.push(
            Resource::new("alice").rate(Money::new(Decimal::from(75), "USD")),
        );

        match resolve_assignment("alice", &project) {
            ResolvedAssignment::Concrete { rate, resource_id } => {
                assert_eq!(resource_id, "alice");
                assert!(rate.is_some());
                assert_eq!(rate.unwrap().amount, Decimal::from(75));
            }
            _ => panic!("Expected concrete assignment"),
        }
    }

    #[test]
    fn resolve_assignment_abstract_profile() {
        let mut project = Project::new("Test");
        project.profiles.push(
            ResourceProfile::new("developer")
                .rate_range(RateRange::new(Decimal::from(60), Decimal::from(100))),
        );

        match resolve_assignment("developer", &project) {
            ResolvedAssignment::Abstract { rate_range, profile_id } => {
                assert_eq!(profile_id, "developer");
                let range = rate_range.unwrap();
                assert_eq!(range.min, Decimal::from(60));
                assert_eq!(range.max, Decimal::from(100));
            }
            _ => panic!("Expected abstract assignment"),
        }
    }

    #[test]
    fn calculate_cost_concrete() {
        use utf8proj_core::Resource;

        let mut project = Project::new("Test");
        project.resources.push(
            Resource::new("alice").rate(Money::new(Decimal::from(100), "USD")),
        );

        let (cost, is_abstract) = calculate_assignment_cost("alice", 1.0, 5, &project);

        assert!(!is_abstract);
        let cost = cost.unwrap();
        // 100 × 1.0 × 5 = 500
        assert_eq!(cost.min, Decimal::from(500));
        assert_eq!(cost.max, Decimal::from(500));
        assert_eq!(cost.expected, Decimal::from(500));
    }

    #[test]
    fn calculate_cost_abstract() {
        let mut project = Project::new("Test");
        project.profiles.push(
            ResourceProfile::new("developer")
                .rate_range(RateRange::new(Decimal::from(50), Decimal::from(100))),
        );

        let (cost, is_abstract) = calculate_assignment_cost("developer", 1.0, 10, &project);

        assert!(is_abstract);
        let cost = cost.unwrap();
        // min: 50 × 1.0 × 10 = 500, max: 100 × 1.0 × 10 = 1000
        assert_eq!(cost.min, Decimal::from(500));
        assert_eq!(cost.max, Decimal::from(1000));
        // expected = midpoint = 750
        assert_eq!(cost.expected, Decimal::from(750));
    }

    #[test]
    fn calculate_cost_with_partial_allocation() {
        use utf8proj_core::Resource;

        let mut project = Project::new("Test");
        project.resources.push(
            Resource::new("bob").rate(Money::new(Decimal::from(200), "EUR")),
        );

        let (cost, is_abstract) = calculate_assignment_cost("bob", 0.5, 4, &project);

        assert!(!is_abstract);
        let cost = cost.unwrap();
        // 200 × 0.5 × 4 = 400
        assert_eq!(cost.min, Decimal::from(400));
        assert_eq!(cost.currency, "EUR");
    }

    #[test]
    fn aggregate_cost_ranges_single() {
        let ranges = vec![CostRange::fixed(Decimal::from(100), "USD")];
        let total = aggregate_cost_ranges(&ranges).unwrap();

        assert_eq!(total.min, Decimal::from(100));
        assert_eq!(total.max, Decimal::from(100));
    }

    #[test]
    fn aggregate_cost_ranges_multiple() {
        let ranges = vec![
            CostRange::new(
                Decimal::from(100),
                Decimal::from(150),
                Decimal::from(200),
                "USD".to_string(),
            ),
            CostRange::new(
                Decimal::from(50),
                Decimal::from(75),
                Decimal::from(100),
                "USD".to_string(),
            ),
        ];
        let total = aggregate_cost_ranges(&ranges).unwrap();

        assert_eq!(total.min, Decimal::from(150));
        assert_eq!(total.expected, Decimal::from(225));
        assert_eq!(total.max, Decimal::from(300));
    }

    #[test]
    fn schedule_with_profile_assignment() {
        let mut project = Project::new("RFC-0001 Test");
        project.start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        project.profiles.push(
            ResourceProfile::new("developer")
                .rate_range(RateRange::new(Decimal::from(100), Decimal::from(200))),
        );
        project.tasks = vec![
            Task::new("task1")
                .duration(Duration::days(5))
                .assign("developer"),
        ];

        let solver = CpmSolver::new();
        let schedule = solver.schedule(&project).unwrap();

        let task = &schedule.tasks["task1"];
        assert!(task.has_abstract_assignments);
        assert!(task.cost_range.is_some());

        let cost = task.cost_range.as_ref().unwrap();
        // 100 × 1.0 × 5 = 500 min, 200 × 1.0 × 5 = 1000 max
        assert_eq!(cost.min, Decimal::from(500));
        assert_eq!(cost.max, Decimal::from(1000));
    }

    #[test]
    fn schedule_with_concrete_assignment() {
        use utf8proj_core::Resource;

        let mut project = Project::new("Concrete Test");
        project.start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        project.resources.push(
            Resource::new("alice").rate(Money::new(Decimal::from(150), "USD")),
        );
        project.tasks = vec![
            Task::new("task1")
                .duration(Duration::days(4))
                .assign("alice"),
        ];

        let solver = CpmSolver::new();
        let schedule = solver.schedule(&project).unwrap();

        let task = &schedule.tasks["task1"];
        assert!(!task.has_abstract_assignments);
        assert!(task.cost_range.is_some());

        let cost = task.cost_range.as_ref().unwrap();
        // 150 × 1.0 × 4 = 600 (fixed)
        assert_eq!(cost.min, Decimal::from(600));
        assert_eq!(cost.max, Decimal::from(600));
    }

    #[test]
    fn schedule_aggregates_total_cost_range() {
        let mut project = Project::new("Aggregate Test");
        project.start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        project.profiles.push(
            ResourceProfile::new("developer")
                .rate_range(RateRange::new(Decimal::from(100), Decimal::from(150))),
        );
        project.tasks = vec![
            Task::new("task1")
                .duration(Duration::days(2))
                .assign("developer"),
            Task::new("task2")
                .duration(Duration::days(3))
                .assign("developer")
                .depends_on("task1"),
        ];

        let solver = CpmSolver::new();
        let schedule = solver.schedule(&project).unwrap();

        assert!(schedule.total_cost_range.is_some());
        let total = schedule.total_cost_range.as_ref().unwrap();
        // task1: 100×2=200 min, 150×2=300 max
        // task2: 100×3=300 min, 150×3=450 max
        // total: 500 min, 750 max
        assert_eq!(total.min, Decimal::from(500));
        assert_eq!(total.max, Decimal::from(750));
    }

    // =============================================================================
    // Diagnostic Analysis Tests
    // =============================================================================

    #[test]
    fn analyze_detects_circular_specialization() {
        use utf8proj_core::CollectingEmitter;

        let mut project = Project::new("Cycle Test");
        project.start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        project.profiles.push(ResourceProfile::new("a").specializes("b"));
        project.profiles.push(ResourceProfile::new("b").specializes("c"));
        project.profiles.push(ResourceProfile::new("c").specializes("a"));

        let mut emitter = CollectingEmitter::new();
        let config = AnalysisConfig::default();
        analyze_project(&project, None, &config, &mut emitter);

        assert!(emitter.has_errors());
        assert!(emitter.diagnostics.iter().any(|d| d.code == DiagnosticCode::E001CircularSpecialization));
    }

    #[test]
    fn analyze_detects_unknown_trait() {
        use utf8proj_core::CollectingEmitter;

        let mut project = Project::new("Unknown Trait Test");
        project.start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        project.profiles.push(
            ResourceProfile::new("dev")
                .rate_range(RateRange::new(Decimal::from(100), Decimal::from(200)))
                .with_trait("nonexistent"),
        );

        let mut emitter = CollectingEmitter::new();
        let config = AnalysisConfig::default();
        analyze_project(&project, None, &config, &mut emitter);

        assert!(emitter.diagnostics.iter().any(|d| d.code == DiagnosticCode::W003UnknownTrait));
    }

    #[test]
    fn analyze_detects_profile_without_rate() {
        use utf8proj_core::CollectingEmitter;

        let mut project = Project::new("No Rate Test");
        project.start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        project.profiles.push(ResourceProfile::new("dev")); // No rate
        project.tasks = vec![
            Task::new("task1").duration(Duration::days(5)).assign("dev"),
        ];

        let mut emitter = CollectingEmitter::new();
        let config = AnalysisConfig::default();
        analyze_project(&project, None, &config, &mut emitter);

        assert!(emitter.diagnostics.iter().any(|d| d.code == DiagnosticCode::E002ProfileWithoutRate));
    }

    #[test]
    fn analyze_detects_abstract_assignment() {
        use utf8proj_core::CollectingEmitter;

        let mut project = Project::new("Abstract Test");
        project.start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        project.profiles.push(
            ResourceProfile::new("developer")
                .rate_range(RateRange::new(Decimal::from(100), Decimal::from(200))),
        );
        project.tasks = vec![
            Task::new("task1").duration(Duration::days(5)).assign("developer"),
        ];

        let mut emitter = CollectingEmitter::new();
        let config = AnalysisConfig::default();
        analyze_project(&project, None, &config, &mut emitter);

        assert!(emitter.diagnostics.iter().any(|d| d.code == DiagnosticCode::W001AbstractAssignment));
    }

    #[test]
    fn analyze_detects_mixed_abstraction() {
        use utf8proj_core::{CollectingEmitter, Resource};

        let mut project = Project::new("Mixed Test");
        project.start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        project.resources.push(Resource::new("alice").rate(Money::new(Decimal::from(100), "USD")));
        project.profiles.push(
            ResourceProfile::new("developer")
                .rate_range(RateRange::new(Decimal::from(100), Decimal::from(200))),
        );
        project.tasks = vec![
            Task::new("task1")
                .duration(Duration::days(5))
                .assign("alice")
                .assign("developer"),
        ];

        let mut emitter = CollectingEmitter::new();
        let config = AnalysisConfig::default();
        analyze_project(&project, None, &config, &mut emitter);

        assert!(emitter.diagnostics.iter().any(|d| d.code == DiagnosticCode::H001MixedAbstraction));
    }

    #[test]
    fn analyze_detects_unused_profile() {
        use utf8proj_core::CollectingEmitter;

        let mut project = Project::new("Unused Profile Test");
        project.start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        project.profiles.push(
            ResourceProfile::new("designer")
                .rate_range(RateRange::new(Decimal::from(80), Decimal::from(120))),
        );
        // No tasks use the profile

        let mut emitter = CollectingEmitter::new();
        let config = AnalysisConfig::default();
        analyze_project(&project, None, &config, &mut emitter);

        assert!(emitter.diagnostics.iter().any(|d| d.code == DiagnosticCode::H002UnusedProfile));
    }

    #[test]
    fn analyze_detects_unused_trait() {
        use utf8proj_core::{CollectingEmitter, Trait};

        let mut project = Project::new("Unused Trait Test");
        project.start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        project.traits.push(Trait::new("senior").rate_multiplier(1.3));
        // No profiles use the trait

        let mut emitter = CollectingEmitter::new();
        let config = AnalysisConfig::default();
        analyze_project(&project, None, &config, &mut emitter);

        assert!(emitter.diagnostics.iter().any(|d| d.code == DiagnosticCode::H003UnusedTrait));
    }

    #[test]
    fn analyze_emits_project_summary() {
        use utf8proj_core::{CollectingEmitter, Resource};

        let mut project = Project::new("Summary Test");
        project.start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        project.resources.push(Resource::new("alice").rate(Money::new(Decimal::from(100), "USD")));
        project.tasks = vec![
            Task::new("task1").duration(Duration::days(5)).assign("alice"),
        ];

        let solver = CpmSolver::new();
        let schedule = solver.schedule(&project).unwrap();

        let mut emitter = CollectingEmitter::new();
        let config = AnalysisConfig::default();
        analyze_project(&project, Some(&schedule), &config, &mut emitter);

        assert!(emitter.diagnostics.iter().any(|d| d.code == DiagnosticCode::I001ProjectCostSummary));
    }

    #[test]
    fn analyze_detects_wide_cost_range() {
        use utf8proj_core::CollectingEmitter;

        let mut project = Project::new("Wide Range Test");
        project.start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        // Rate range 50-200, expected=125, half_spread=75, spread=60% (exceeds 50%)
        project.profiles.push(
            ResourceProfile::new("developer")
                .rate_range(RateRange::new(Decimal::from(50), Decimal::from(200))),
        );
        project.tasks = vec![
            Task::new("task1").duration(Duration::days(10)).assign("developer"),
        ];

        let solver = CpmSolver::new();
        let schedule = solver.schedule(&project).unwrap();

        let mut emitter = CollectingEmitter::new();
        let config = AnalysisConfig::default().with_cost_spread_threshold(50.0);
        analyze_project(&project, Some(&schedule), &config, &mut emitter);

        assert!(emitter.diagnostics.iter().any(|d| d.code == DiagnosticCode::W002WideCostRange));
    }

    #[test]
    fn analyze_no_wide_cost_range_under_threshold() {
        use utf8proj_core::CollectingEmitter;

        let mut project = Project::new("Narrow Range Test");
        project.start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        // Rate range 90-110 has ~20% spread (under 50% threshold)
        project.profiles.push(
            ResourceProfile::new("developer")
                .rate_range(RateRange::new(Decimal::from(90), Decimal::from(110))),
        );
        project.tasks = vec![
            Task::new("task1").duration(Duration::days(10)).assign("developer"),
        ];

        let solver = CpmSolver::new();
        let schedule = solver.schedule(&project).unwrap();

        let mut emitter = CollectingEmitter::new();
        let config = AnalysisConfig::default();
        analyze_project(&project, Some(&schedule), &config, &mut emitter);

        assert!(!emitter.diagnostics.iter().any(|d| d.code == DiagnosticCode::W002WideCostRange));
    }

    #[test]
    fn analysis_config_builder() {
        let config = AnalysisConfig::new()
            .with_file("test.proj")
            .with_cost_spread_threshold(75.0);

        assert_eq!(config.file, Some(PathBuf::from("test.proj")));
        assert_eq!(config.cost_spread_threshold, 75.0);
    }

    // =========================================================================
    // Coverage: Semantic Gap Tests
    // =========================================================================

    #[test]
    fn summary_unknown_cost_when_no_rate_data() {
        use utf8proj_core::CollectingEmitter;

        // Project with no rates at all - cost should be "unknown"
        let mut project = Project::new("No Cost Data Test");
        project.start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        project.tasks = vec![Task::new("task1").duration(Duration::days(5))];

        let solver = CpmSolver::new();
        let schedule = solver.schedule(&project).unwrap();

        // Verify no cost range
        assert!(schedule.total_cost_range.is_none());

        let mut emitter = CollectingEmitter::new();
        let config = AnalysisConfig::default();
        analyze_project(&project, Some(&schedule), &config, &mut emitter);

        // Should emit I001 with "unknown (no cost data)"
        let summary = emitter
            .diagnostics
            .iter()
            .find(|d| d.code == DiagnosticCode::I001ProjectCostSummary)
            .expect("Should have I001");
        assert!(summary.notes.iter().any(|n| n.contains("unknown")));
    }

    #[test]
    fn e002_with_many_tasks_truncates_list() {
        use utf8proj_core::CollectingEmitter;

        // Profile without rate assigned to more than 3 tasks
        let mut project = Project::new("Many Tasks Test");
        project.start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        project.profiles.push(ResourceProfile::new("dev")); // No rate
        project.tasks = vec![
            Task::new("task1").duration(Duration::days(1)).assign("dev"),
            Task::new("task2").duration(Duration::days(1)).assign("dev"),
            Task::new("task3").duration(Duration::days(1)).assign("dev"),
            Task::new("task4").duration(Duration::days(1)).assign("dev"),
            Task::new("task5").duration(Duration::days(1)).assign("dev"),
        ];

        let mut emitter = CollectingEmitter::new();
        let config = AnalysisConfig::default();
        analyze_project(&project, None, &config, &mut emitter);

        let e002 = emitter
            .diagnostics
            .iter()
            .find(|d| d.code == DiagnosticCode::E002ProfileWithoutRate)
            .expect("Should have E002");
        // Should truncate: "task1, task2, ... (5 tasks)"
        assert!(e002.notes.iter().any(|n| n.contains("5 tasks")));
    }

    #[test]
    fn w002_with_trait_multiplier_contributor() {
        use utf8proj_core::{CollectingEmitter, Trait};

        // Wide range amplified by trait multiplier should list it as contributor
        let mut project = Project::new("Trait Contributor Test");
        project.start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        // Define a significant trait multiplier
        project.traits.push(Trait::new("senior").rate_multiplier(1.5));
        // Rate range that becomes wide after trait: 50-200 * 1.5 = 75-300
        project.profiles.push(
            ResourceProfile::new("developer")
                .rate_range(RateRange::new(Decimal::from(50), Decimal::from(200)))
                .with_trait("senior"),
        );
        project.tasks = vec![Task::new("task1")
            .duration(Duration::days(10))
            .assign("developer")];

        let solver = CpmSolver::new();
        let schedule = solver.schedule(&project).unwrap();

        let mut emitter = CollectingEmitter::new();
        let config = AnalysisConfig::default().with_cost_spread_threshold(50.0);
        analyze_project(&project, Some(&schedule), &config, &mut emitter);

        let w002 = emitter
            .diagnostics
            .iter()
            .find(|d| d.code == DiagnosticCode::W002WideCostRange)
            .expect("Should have W002");
        // Should mention the trait as a contributor
        assert!(w002
            .notes
            .iter()
            .any(|n| n.contains("senior") && n.contains("multiplier")));
    }

    #[test]
    fn profile_with_fixed_rate_converts_to_range() {
        use utf8proj_core::CollectingEmitter;

        // Profile with fixed rate (not range) - should work in analysis
        let mut project = Project::new("Fixed Rate Test");
        project.start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        project.profiles.push(
            ResourceProfile::new("developer").rate(Money::new(Decimal::from(100), "USD")),
        );
        project.tasks = vec![Task::new("task1")
            .duration(Duration::days(5))
            .assign("developer")];

        let solver = CpmSolver::new();
        let schedule = solver.schedule(&project).unwrap();

        let mut emitter = CollectingEmitter::new();
        let config = AnalysisConfig::default();
        analyze_project(&project, Some(&schedule), &config, &mut emitter);

        // Should have W001 for abstract assignment
        assert!(emitter
            .diagnostics
            .iter()
            .any(|d| d.code == DiagnosticCode::W001AbstractAssignment));
        // Cost should be calculated (fixed rate = 100, 5 days = 500)
        let summary = emitter
            .diagnostics
            .iter()
            .find(|d| d.code == DiagnosticCode::I001ProjectCostSummary)
            .expect("Should have I001");
        assert!(summary.notes.iter().any(|n| n.contains("$500")));
    }

    #[test]
    fn specializes_inherits_rate_from_parent() {
        use utf8proj_core::CollectingEmitter;

        // Child profile inherits rate from parent
        let mut project = Project::new("Specialization Test");
        project.start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        project.profiles.push(
            ResourceProfile::new("developer")
                .rate_range(RateRange::new(Decimal::from(100), Decimal::from(200))),
        );
        project.profiles.push(ResourceProfile::new("senior_developer").specializes("developer"));
        project.tasks = vec![Task::new("task1")
            .duration(Duration::days(5))
            .assign("senior_developer")];

        let solver = CpmSolver::new();
        let schedule = solver.schedule(&project).unwrap();

        let mut emitter = CollectingEmitter::new();
        let config = AnalysisConfig::default();
        analyze_project(&project, Some(&schedule), &config, &mut emitter);

        // Should NOT emit E002 since rate is inherited
        assert!(!emitter
            .diagnostics
            .iter()
            .any(|d| d.code == DiagnosticCode::E002ProfileWithoutRate));
        // Should have cost range in summary (inherited 100-200)
        let summary = emitter
            .diagnostics
            .iter()
            .find(|d| d.code == DiagnosticCode::I001ProjectCostSummary)
            .expect("Should have I001");
        assert!(summary.notes.iter().any(|n| n.contains("$") && n.contains("-")));
    }

    #[test]
    fn nested_child_task_with_profile_assignment() {
        use utf8proj_core::CollectingEmitter;

        // Profile assigned to nested task
        let mut project = Project::new("Nested Assignment Test");
        project.start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        project.profiles.push(
            ResourceProfile::new("developer")
                .rate_range(RateRange::new(Decimal::from(100), Decimal::from(200))),
        );
        project.tasks = vec![Task::new("phase1")
            .child(
                Task::new("design")
                    .duration(Duration::days(3))
                    .assign("developer"),
            )
            .child(
                Task::new("implement")
                    .duration(Duration::days(5))
                    .assign("developer"),
            )];

        let mut emitter = CollectingEmitter::new();
        let config = AnalysisConfig::default();
        analyze_project(&project, None, &config, &mut emitter);

        // Should detect abstract assignments for both nested tasks
        let w001_count = emitter
            .diagnostics
            .iter()
            .filter(|d| d.code == DiagnosticCode::W001AbstractAssignment)
            .count();
        assert_eq!(w001_count, 2);
    }

    #[test]
    fn abstract_assignment_without_rate_shows_unknown_cost() {
        use utf8proj_core::CollectingEmitter;

        // Profile without rate assigned - cost should say "unknown"
        let mut project = Project::new("Unknown Cost Range Test");
        project.start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        project.profiles.push(ResourceProfile::new("developer")); // No rate
        project.tasks = vec![Task::new("task1")
            .duration(Duration::days(5))
            .assign("developer")];

        let mut emitter = CollectingEmitter::new();
        let config = AnalysisConfig::default();
        analyze_project(&project, None, &config, &mut emitter);

        let w001 = emitter
            .diagnostics
            .iter()
            .find(|d| d.code == DiagnosticCode::W001AbstractAssignment)
            .expect("Should have W001");
        assert!(w001.notes.iter().any(|n| n.contains("unknown")));
    }

    // =========================================================================
    // Semantic Gap Coverage: Edge Cases
    // =========================================================================

    #[test]
    fn isolated_task_no_predecessors_or_successors() {
        // Test CPM with task that has neither predecessors nor successors
        // Covers: cpm.rs lines 125, 158, 208
        let mut project = Project::new("Isolated Task Test");
        project.start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        project.tasks = vec![Task::new("alone").duration(Duration::days(3))];

        let solver = CpmSolver::new();
        let schedule = solver.schedule(&project).unwrap();

        // Single task is its own critical path
        assert_eq!(schedule.tasks.len(), 1);
        assert!(schedule.tasks["alone"].is_critical);
        assert_eq!(schedule.tasks["alone"].slack, Duration::zero());
    }

    #[test]
    fn parallel_tasks_no_dependencies() {
        // Multiple tasks with no dependencies - all start day 0
        // Covers: cpm.rs line 125 (no predecessors)
        let mut project = Project::new("Parallel Test");
        project.start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        project.tasks = vec![
            Task::new("a").duration(Duration::days(5)),
            Task::new("b").duration(Duration::days(3)),
            Task::new("c").duration(Duration::days(7)),
        ];

        let solver = CpmSolver::new();
        let schedule = solver.schedule(&project).unwrap();

        // All tasks start on the same day
        let start = project.start;
        assert_eq!(schedule.tasks["a"].start, start);
        assert_eq!(schedule.tasks["b"].start, start);
        assert_eq!(schedule.tasks["c"].start, start);

        // Longest task (c) is critical
        assert!(schedule.tasks["c"].is_critical);
        // Shorter tasks have slack
        assert!(schedule.tasks["a"].slack.minutes > 0);
        assert!(schedule.tasks["b"].slack.minutes > 0);
    }

    #[test]
    fn task_with_no_successors_uses_project_end() {
        // Task at the end of chain has no successors
        // Covers: cpm.rs lines 158, 208 (no successors branch)
        let mut project = Project::new("Terminal Task Test");
        project.start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        project.tasks = vec![
            Task::new("first").duration(Duration::days(5)),
            Task::new("last").duration(Duration::days(3)).depends_on("first"),
        ];

        let solver = CpmSolver::new();
        let schedule = solver.schedule(&project).unwrap();

        // Both tasks are critical (sequential chain)
        assert!(schedule.tasks["first"].is_critical);
        assert!(schedule.tasks["last"].is_critical);

        // Free slack of last task equals total slack (no successors)
        let last_task = &schedule.tasks["last"];
        assert_eq!(last_task.slack, Duration::zero()); // Critical path
    }

    #[test]
    fn relative_resolution_empty_container() {
        // Test relative dependency resolution when task is at root level
        // Covers: lib.rs line 197 (empty container path)
        let mut project = Project::new("Root Level Test");
        project.start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        project.tasks = vec![
            Task::new("root_a").duration(Duration::days(3)),
            Task::new("root_b").duration(Duration::days(2)).depends_on("root_a"),
        ];

        let solver = CpmSolver::new();
        let schedule = solver.schedule(&project).unwrap();

        // Dependency resolved correctly at root level
        assert!(schedule.tasks["root_b"].start >= schedule.tasks["root_a"].finish);
    }

    #[test]
    fn resolve_assignment_unknown_id() {
        // Test assigning to an ID that's neither a resource nor profile
        // Covers: lib.rs lines 486-489 (unknown assignment)
        let mut project = Project::new("Unknown Assignment Test");
        project.start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        project.tasks = vec![Task::new("task1")
            .duration(Duration::days(5))
            .assign("nonexistent_entity")];

        let solver = CpmSolver::new();
        let schedule = solver.schedule(&project).unwrap();

        // Should still schedule (unknown treated as concrete with no rate)
        assert_eq!(schedule.tasks.len(), 1);
    }

    #[test]
    fn abstract_profile_with_no_rate_in_cost_calculation() {
        // Test cost calculation when abstract profile has no rate
        // Covers: lib.rs line 574 (None rate range)
        let mut project = Project::new("No Rate Profile Test");
        project.start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();

        // Profile with no rate (not inherited either)
        project.profiles.push(ResourceProfile::new("bare_profile"));

        project.tasks = vec![Task::new("work")
            .duration(Duration::days(5))
            .assign("bare_profile")];

        let solver = CpmSolver::new();
        let schedule = solver.schedule(&project).unwrap();

        // Schedule should work, but cost range is None
        assert!(schedule.total_cost_range.is_none());
    }

    #[test]
    fn working_day_cache_large_project() {
        // Test with a project that might exceed working day cache
        // Covers: lib.rs line 280 (cache beyond limit fallback)
        let mut project = Project::new("Large Duration Test");
        project.start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();

        // Create a very long task (2000+ days)
        project.tasks = vec![Task::new("marathon").duration(Duration::days(2500))];

        let solver = CpmSolver::new();
        let schedule = solver.schedule(&project).unwrap();

        // Should handle gracefully even if cache is exceeded
        assert_eq!(schedule.tasks.len(), 1);
        assert!(schedule.project_duration.minutes >= Duration::days(2500).minutes);
    }
}
