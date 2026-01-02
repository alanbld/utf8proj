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
    Assignment, Calendar, Duration, Explanation, FeasibilityResult, Project, Schedule,
    ScheduleError, ScheduledTask, Scheduler, Task, TaskConstraint, TaskId,
};

/// CPM-based scheduler
pub struct CpmSolver {
    /// Whether to perform resource leveling
    pub resource_leveling: bool,
}

impl CpmSolver {
    pub fn new() -> Self {
        Self {
            resource_leveling: false, // Not implemented yet
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

/// Flatten the hierarchical task tree into a HashMap
fn flatten_tasks<'a>(tasks: &'a [Task], map: &mut HashMap<String, &'a Task>) {
    for task in tasks {
        map.insert(task.id.clone(), task);
        if !task.children.is_empty() {
            flatten_tasks(&task.children, map);
        }
    }
}

/// Get the duration of a task in working days
fn get_task_duration_days(task: &Task) -> i64 {
    // If explicit duration is set, use that
    if let Some(dur) = task.duration {
        return dur.as_days().ceil() as i64;
    }
    // Otherwise use effort (assuming 1 resource at 100%)
    if let Some(effort) = task.effort {
        return effort.as_days().ceil() as i64;
    }
    // Milestone or summary task
    0
}

/// Add working days to a date, respecting the calendar
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

/// Convert days from project start to an actual date
fn days_to_date(project_start: NaiveDate, days: i64, calendar: &Calendar) -> NaiveDate {
    add_working_days(project_start, days, calendar)
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

/// Perform topological sort using Kahn's algorithm
/// Returns sorted task IDs or error if cycle detected
fn topological_sort(
    tasks: &HashMap<String, &Task>,
) -> Result<Vec<String>, ScheduleError> {
    // Build adjacency list and in-degree count
    let mut in_degree: HashMap<String, usize> = HashMap::new();
    let mut adjacency: HashMap<String, Vec<String>> = HashMap::new();

    // Initialize all tasks with 0 in-degree
    for id in tasks.keys() {
        in_degree.insert(id.clone(), 0);
        adjacency.insert(id.clone(), Vec::new());
    }

    // Build the graph
    for (id, task) in tasks {
        for dep in &task.depends {
            // dep.predecessor -> id (predecessor must come before this task)
            if let Some(adj) = adjacency.get_mut(&dep.predecessor) {
                adj.push(id.clone());
            }
            if let Some(deg) = in_degree.get_mut(id) {
                *deg += 1;
            }
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

    Ok(result)
}

// =============================================================================
// CPM Implementation
// =============================================================================

impl Scheduler for CpmSolver {
    fn schedule(&self, project: &Project) -> Result<Schedule, ScheduleError> {
        // Step 1: Flatten tasks
        let mut task_map: HashMap<String, &Task> = HashMap::new();
        flatten_tasks(&project.tasks, &mut task_map);

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

        // Step 2: Topological sort
        let sorted_ids = topological_sort(&task_map)?;

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

        // Step 5: Forward pass - calculate ES and EF
        for id in &sorted_ids {
            let task = task_map[id];
            let duration = nodes[id].duration_days;

            // ES = max(EF of all predecessors), or 0 if no predecessors
            let mut es = 0i64;
            for dep in &task.depends {
                if let Some(pred_node) = nodes.get(&dep.predecessor) {
                    es = es.max(pred_node.early_finish);
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

        // Step 6: Find project end (max EF)
        let project_end_days = nodes.values().map(|n| n.early_finish).max().unwrap_or(0);

        // Step 7: Backward pass - calculate LS and LF
        // Process in reverse topological order
        for id in sorted_ids.iter().rev() {
            let duration = nodes[id].duration_days;

            // Find all successors of this task
            let successors: Vec<String> = task_map
                .iter()
                .filter(|(_, t)| t.depends.iter().any(|d| d.predecessor == *id))
                .map(|(id, _)| id.clone())
                .collect();

            // LF = min(LS of all successors), or project_end if no successors
            let lf = if successors.is_empty() {
                project_end_days
            } else {
                successors
                    .iter()
                    .filter_map(|s| nodes.get(s).map(|n| n.late_start))
                    .min()
                    .unwrap_or(project_end_days)
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

        // Step 8: Identify critical path (tasks with zero slack)
        let mut critical_path: Vec<TaskId> = nodes
            .iter()
            .filter(|(_, node)| node.slack == 0 && node.duration_days > 0)
            .map(|(id, _)| id.clone())
            .collect();

        // Sort critical path in topological order
        critical_path.sort_by_key(|id| sorted_ids.iter().position(|s| s == id).unwrap_or(0));

        // Step 9: Build ScheduledTask entries
        let mut scheduled_tasks: HashMap<TaskId, ScheduledTask> = HashMap::new();

        for (id, node) in &nodes {
            let start_date = days_to_date(project.start, node.early_start, &calendar);
            // Finish date is the last day of work, not the day after
            // So for a 20-day task starting Feb 03, finish is Feb 28 (day 20), not Mar 03
            let finish_date = if node.duration_days > 0 {
                // early_finish - 1 because finish is inclusive (last day of work)
                days_to_date(project.start, node.early_finish - 1, &calendar)
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
                    early_start: days_to_date(project.start, node.early_start, &calendar),
                    early_finish: if node.duration_days > 0 {
                        days_to_date(project.start, node.early_finish - 1, &calendar)
                    } else {
                        days_to_date(project.start, node.early_finish, &calendar)
                    },
                    late_start: days_to_date(project.start, node.late_start, &calendar),
                    late_finish: if node.duration_days > 0 {
                        days_to_date(project.start, node.late_finish - 1, &calendar)
                    } else {
                        days_to_date(project.start, node.late_finish, &calendar)
                    },
                },
            );
        }

        // Step 10: Build final schedule
        // project_end is the last working day of the project
        let project_end_date = if project_end_days > 0 {
            days_to_date(project.start, project_end_days - 1, &calendar)
        } else {
            project.start
        };

        Ok(Schedule {
            tasks: scheduled_tasks,
            critical_path,
            project_duration: Duration::days(project_end_days),
            project_end: project_end_date,
            total_cost: None, // Cost calculation would go here
        })
    }

    fn is_feasible(&self, project: &Project) -> FeasibilityResult {
        // Try to schedule and check for errors
        let mut task_map: HashMap<String, &Task> = HashMap::new();
        flatten_tasks(&project.tasks, &mut task_map);

        match topological_sort(&task_map) {
            Ok(_) => FeasibilityResult {
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
        project.tasks = vec![Task::new("phase1")
            .child(Task::new("design").effort(Duration::days(3)))
            .child(
                Task::new("implement")
                    .effort(Duration::days(5))
                    .depends_on("design"),
            )];

        let solver = CpmSolver::new();
        let schedule = solver.schedule(&project).unwrap();

        // Should include all tasks (parent + children)
        assert!(schedule.tasks.contains_key("phase1"));
        assert!(schedule.tasks.contains_key("design"));
        assert!(schedule.tasks.contains_key("implement"));
    }
}
