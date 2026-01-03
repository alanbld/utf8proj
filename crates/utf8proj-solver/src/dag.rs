//! Dependency graph construction and validation
//!
//! This module handles the critical transformation from hierarchical WBS
//! to flat DAG suitable for CPM scheduling.
//!
//! Key principle: The WBS (Work Breakdown Structure) is for PRESENTATION.
//! The DAG (Directed Acyclic Graph) is for SCHEDULING.
//! These must be completely separated.

use std::collections::{HashMap, VecDeque};
use utf8proj_core::{DependencyType, Duration, Task, TaskId};

/// Errors during graph construction
#[derive(Debug, Clone, PartialEq)]
pub enum GraphError {
    /// Cycle detected in dependencies
    CycleDetected { tasks: Vec<TaskId> },
    /// Referenced task doesn't exist
    MissingDependency { task: TaskId, missing: TaskId },
}

impl std::fmt::Display for GraphError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GraphError::CycleDetected { tasks } => {
                write!(f, "Cycle detected involving tasks: {:?}", tasks)
            }
            GraphError::MissingDependency { task, missing } => {
                write!(f, "Task '{}' depends on '{}' which doesn't exist", task, missing)
            }
        }
    }
}

impl std::error::Error for GraphError {}

/// Dependency declaration from original task
#[derive(Debug, Clone)]
pub struct LeafDependency {
    /// Predecessor reference (may be qualified or simple)
    pub predecessor: String,
    /// Dependency type
    pub dep_type: DependencyType,
    /// Lag in working days
    pub lag_days: i64,
}

/// A leaf task extracted from the WBS
#[derive(Debug, Clone)]
pub struct LeafTask {
    /// Task identifier
    pub id: TaskId,
    /// Display name
    pub name: String,
    /// Duration in working days (computed from effort if needed)
    pub duration_days: i64,
    /// Original effort (person-time)
    pub effort: Option<Duration>,
    /// Assigned resource IDs with units
    pub assigned: Vec<(String, f32)>,
    /// Path from root (for WBS reconstruction)
    pub wbs_path: Vec<TaskId>,
    /// Original task reference path (qualified ID)
    pub qualified_id: String,
    /// Whether this is a milestone
    pub is_milestone: bool,
    /// Completion percentage
    pub complete: Option<f32>,
    /// Dependencies from original task
    pub dependencies: Vec<LeafDependency>,
}

/// An edge in the dependency graph
#[derive(Debug, Clone)]
pub struct DependencyEdge {
    /// Predecessor task ID
    pub from: TaskId,
    /// Successor task ID
    pub to: TaskId,
    /// Dependency type
    pub dep_type: DependencyType,
    /// Lag in working days (can be negative for lead time)
    pub lag_days: i64,
}

/// A flattened, schedulable graph of leaf tasks only
#[derive(Debug)]
pub struct SchedulingGraph {
    /// All leaf tasks (no containers)
    pub tasks: Vec<LeafTask>,
    /// Task lookup by ID
    pub task_map: HashMap<TaskId, usize>,
    /// Adjacency list: task_id -> list of successor task_ids with edges
    pub successors: HashMap<TaskId, Vec<DependencyEdge>>,
    /// Reverse adjacency: task_id -> list of predecessor task_ids with edges
    pub predecessors: HashMap<TaskId, Vec<DependencyEdge>>,
    /// Topological order (computed once, reused)
    pub topo_order: Vec<TaskId>,
    /// Map from qualified ID to simple ID (for container date derivation)
    pub qualified_to_simple: HashMap<String, TaskId>,
}

impl SchedulingGraph {
    /// Flatten a WBS tree into a scheduling graph
    pub fn from_wbs(tasks: &[Task]) -> Result<Self, GraphError> {
        let mut leaf_tasks = Vec::new();
        let mut qualified_to_simple: HashMap<String, TaskId> = HashMap::new();

        // 1. Collect all leaf tasks
        collect_leaves(tasks, &mut leaf_tasks, &mut qualified_to_simple, vec![], "");

        // Build task lookup
        let task_map: HashMap<TaskId, usize> = leaf_tasks
            .iter()
            .enumerate()
            .map(|(i, t)| (t.id.clone(), i))
            .collect();

        // 2. Build a map of all task IDs (for container expansion)
        let mut all_tasks_map: HashMap<String, Vec<TaskId>> = HashMap::new();
        build_container_map(tasks, &mut all_tasks_map, "");

        // 3. Resolve all dependencies to leaf-to-leaf edges
        let mut successors: HashMap<TaskId, Vec<DependencyEdge>> = HashMap::new();
        let mut predecessors: HashMap<TaskId, Vec<DependencyEdge>> = HashMap::new();

        // Initialize empty lists
        for task in &leaf_tasks {
            successors.insert(task.id.clone(), Vec::new());
            predecessors.insert(task.id.clone(), Vec::new());
        }

        // Build edges
        for task in &leaf_tasks {
            let resolved_deps = resolve_task_dependencies(
                task,
                &qualified_to_simple,
                &all_tasks_map,
                &task_map,
            )?;

            for (pred_id, dep_type, lag_days) in resolved_deps {
                let edge = DependencyEdge {
                    from: pred_id.clone(),
                    to: task.id.clone(),
                    dep_type,
                    lag_days,
                };

                successors.get_mut(&pred_id).unwrap().push(edge.clone());
                predecessors.get_mut(&task.id).unwrap().push(edge);
            }
        }

        // 4. Compute topological order (also validates acyclicity)
        let topo_order = topological_sort(&leaf_tasks, &successors)?;

        Ok(Self {
            tasks: leaf_tasks,
            task_map,
            successors,
            predecessors,
            topo_order,
            qualified_to_simple,
        })
    }

    /// Get a leaf task by ID
    pub fn get_task(&self, id: &str) -> Option<&LeafTask> {
        self.task_map.get(id).map(|&i| &self.tasks[i])
    }
}

/// Collect all leaf tasks from the WBS hierarchy
fn collect_leaves(
    tasks: &[Task],
    leaves: &mut Vec<LeafTask>,
    qualified_map: &mut HashMap<String, TaskId>,
    path: Vec<TaskId>,
    prefix: &str,
) {
    for task in tasks {
        let qualified_id = if prefix.is_empty() {
            task.id.clone()
        } else {
            format!("{}.{}", prefix, task.id)
        };

        let mut current_path = path.clone();
        current_path.push(task.id.clone());

        if task.children.is_empty() {
            // Leaf task
            let duration_days = compute_duration_days(task);

            // Collect dependencies with lag
            let dependencies: Vec<LeafDependency> = task.depends.iter()
                .map(|dep| LeafDependency {
                    predecessor: dep.predecessor.clone(),
                    dep_type: dep.dep_type,
                    lag_days: dep.lag.map(|d| d.as_days() as i64).unwrap_or(0),
                })
                .collect();

            leaves.push(LeafTask {
                id: task.id.clone(),
                name: task.name.clone(),
                duration_days,
                effort: task.effort,
                assigned: task.assigned.iter()
                    .map(|a| (a.resource_id.clone(), a.units))
                    .collect(),
                wbs_path: current_path,
                qualified_id: qualified_id.clone(),
                is_milestone: task.milestone,
                complete: task.complete,
                dependencies,
            });

            qualified_map.insert(qualified_id, task.id.clone());
        } else {
            // Container - recurse into children
            qualified_map.insert(qualified_id.clone(), task.id.clone());
            collect_leaves(&task.children, leaves, qualified_map, current_path, &qualified_id);
        }
    }
}

/// Build a map from container qualified ID to all leaf task IDs under it
fn build_container_map(
    tasks: &[Task],
    map: &mut HashMap<String, Vec<TaskId>>,
    prefix: &str,
) {
    for task in tasks {
        let qualified_id = if prefix.is_empty() {
            task.id.clone()
        } else {
            format!("{}.{}", prefix, task.id)
        };

        if task.children.is_empty() {
            // Leaf task - add to all parent containers
            map.entry(qualified_id.clone()).or_default().push(task.id.clone());

            // Also add to parent containers
            let mut container_path = prefix.to_string();
            for part in prefix.split('.').filter(|s| !s.is_empty()) {
                if container_path.is_empty() {
                    container_path = part.to_string();
                }
                map.entry(container_path.clone()).or_default().push(task.id.clone());
            }
        } else {
            // Container - recurse
            build_container_map(&task.children, map, &qualified_id);

            // Collect all leaves under this container
            let mut all_leaves = Vec::new();
            collect_container_leaves(&task.children, &mut all_leaves, &qualified_id);
            map.insert(qualified_id, all_leaves);
        }
    }
}

/// Helper to collect all leaf IDs under a container
fn collect_container_leaves(tasks: &[Task], leaves: &mut Vec<TaskId>, _prefix: &str) {
    for task in tasks {
        if task.children.is_empty() {
            leaves.push(task.id.clone());
        } else {
            collect_container_leaves(&task.children, leaves, _prefix);
        }
    }
}

/// Compute duration in working days for a task
fn compute_duration_days(task: &Task) -> i64 {
    // If explicit duration is set, use that
    if let Some(dur) = task.duration {
        return dur.as_days().ceil() as i64;
    }

    // Effort-driven: Duration = Effort / Total_Resource_Units
    if let Some(effort) = task.effort {
        let total_units: f64 = if task.assigned.is_empty() {
            1.0
        } else {
            task.assigned.iter().map(|r| r.units as f64).sum()
        };

        let effective_units = if total_units > 0.0 { total_units } else { 1.0 };
        return (effort.as_days() / effective_units).ceil() as i64;
    }

    // Milestone or summary
    0
}

/// Resolve dependencies for a leaf task
///
/// Handles:
/// - Simple IDs: "task_a" -> resolved to leaf task
/// - Qualified paths: "phase1.task_a" -> resolved to leaf task
/// - Container references: "phase1" -> expanded to all leaves under phase1
fn resolve_task_dependencies(
    task: &LeafTask,
    qualified_map: &HashMap<String, TaskId>,
    container_map: &HashMap<String, Vec<TaskId>>,
    task_map: &HashMap<TaskId, usize>,
) -> Result<Vec<(TaskId, DependencyType, i64)>, GraphError> {
    let mut resolved = Vec::new();

    for dep in &task.dependencies {
        let pred_ref = &dep.predecessor;

        // Try to resolve the dependency
        // 1. Check if it's a simple task ID (leaf task)
        if task_map.contains_key(pred_ref) {
            resolved.push((pred_ref.clone(), dep.dep_type, dep.lag_days));
            continue;
        }

        // 2. Check if it's a qualified path that maps to a leaf
        if let Some(simple_id) = qualified_map.get(pred_ref) {
            if task_map.contains_key(simple_id) {
                resolved.push((simple_id.clone(), dep.dep_type, dep.lag_days));
                continue;
            }
        }

        // 3. Check if it's a container - expand to all leaves under it
        if let Some(leaves) = container_map.get(pred_ref) {
            for leaf_id in leaves {
                if task_map.contains_key(leaf_id) {
                    resolved.push((leaf_id.clone(), dep.dep_type, dep.lag_days));
                }
            }
            continue;
        }

        // 4. Try relative resolution (sibling in same container)
        // Build the qualified path by prepending the task's container prefix
        let container_prefix = task.qualified_id.rsplit_once('.')
            .map(|(prefix, _)| prefix)
            .unwrap_or("");

        if !container_prefix.is_empty() {
            let qualified_pred = format!("{}.{}", container_prefix, pred_ref);

            // Check if qualified path is a leaf
            if let Some(simple_id) = qualified_map.get(&qualified_pred) {
                if task_map.contains_key(simple_id) {
                    resolved.push((simple_id.clone(), dep.dep_type, dep.lag_days));
                    continue;
                }
            }

            // Check if qualified path is a container
            if let Some(leaves) = container_map.get(&qualified_pred) {
                for leaf_id in leaves {
                    if task_map.contains_key(leaf_id) {
                        resolved.push((leaf_id.clone(), dep.dep_type, dep.lag_days));
                    }
                }
                continue;
            }
        }

        // Dependency couldn't be resolved - this might be an error
        // For now, we skip it (the original code did this too)
        // TODO: Return error for missing dependencies
    }

    Ok(resolved)
}

/// Kahn's algorithm for topological sort
fn topological_sort(
    tasks: &[LeafTask],
    successors: &HashMap<TaskId, Vec<DependencyEdge>>,
) -> Result<Vec<TaskId>, GraphError> {
    let mut in_degree: HashMap<TaskId, usize> = HashMap::new();

    // Initialize in-degrees to 0
    for task in tasks {
        in_degree.insert(task.id.clone(), 0);
    }

    // Count incoming edges
    for edges in successors.values() {
        for edge in edges {
            *in_degree.get_mut(&edge.to).unwrap() += 1;
        }
    }

    // Start with zero in-degree nodes
    let mut queue: VecDeque<TaskId> = in_degree
        .iter()
        .filter(|(_, &deg)| deg == 0)
        .map(|(id, _)| id.clone())
        .collect();

    let mut result: Vec<TaskId> = Vec::new();

    while let Some(task_id) = queue.pop_front() {
        result.push(task_id.clone());

        if let Some(edges) = successors.get(&task_id) {
            for edge in edges {
                let deg = in_degree.get_mut(&edge.to).unwrap();
                *deg -= 1;
                if *deg == 0 {
                    queue.push_back(edge.to.clone());
                }
            }
        }
    }

    // Check for cycle
    if result.len() != tasks.len() {
        let remaining: Vec<TaskId> = tasks
            .iter()
            .filter(|t| !result.contains(&t.id))
            .map(|t| t.id.clone())
            .collect();
        return Err(GraphError::CycleDetected { tasks: remaining });
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use utf8proj_core::Task;

    #[test]
    fn test_collect_leaves_flat() {
        let tasks = vec![
            Task::new("a").name("Task A").duration(Duration::days(5)),
            Task::new("b").name("Task B").duration(Duration::days(3)),
        ];

        let graph = SchedulingGraph::from_wbs(&tasks).unwrap();

        assert_eq!(graph.tasks.len(), 2);
        assert!(graph.get_task("a").is_some());
        assert!(graph.get_task("b").is_some());
    }

    #[test]
    fn test_collect_leaves_nested() {
        let tasks = vec![
            Task::new("phase1")
                .name("Phase 1")
                .child(Task::new("a").name("Task A").duration(Duration::days(5)))
                .child(Task::new("b").name("Task B").duration(Duration::days(3))),
            Task::new("phase2")
                .name("Phase 2")
                .child(Task::new("c").name("Task C").duration(Duration::days(2))),
        ];

        let graph = SchedulingGraph::from_wbs(&tasks).unwrap();

        // Only leaf tasks
        assert_eq!(graph.tasks.len(), 3);
        assert!(graph.get_task("a").is_some());
        assert!(graph.get_task("b").is_some());
        assert!(graph.get_task("c").is_some());

        // Containers are NOT in the graph
        assert!(graph.get_task("phase1").is_none());
        assert!(graph.get_task("phase2").is_none());
    }

    #[test]
    fn test_qualified_id_mapping() {
        let tasks = vec![
            Task::new("phase1")
                .child(Task::new("a").duration(Duration::days(1))),
        ];

        let graph = SchedulingGraph::from_wbs(&tasks).unwrap();

        let task_a = graph.get_task("a").unwrap();
        assert_eq!(task_a.qualified_id, "phase1.a");
        assert_eq!(task_a.wbs_path, vec!["phase1".to_string(), "a".to_string()]);
    }

    #[test]
    fn test_topological_sort_simple() {
        let tasks = vec![
            Task::new("a").duration(Duration::days(1)),
            Task::new("b").duration(Duration::days(1)),
        ];

        let graph = SchedulingGraph::from_wbs(&tasks).unwrap();

        // No dependencies - both should be in topo order
        assert_eq!(graph.topo_order.len(), 2);
    }

    #[test]
    fn test_duration_calculation_effort() {
        let task = Task::new("work")
            .effort(Duration::days(10))
            .assign_with_units("dev", 0.5);

        // 10 days effort at 50% = 20 days duration
        let duration = compute_duration_days(&task);
        assert_eq!(duration, 20);
    }

    #[test]
    fn test_duration_calculation_explicit() {
        let task = Task::new("work").duration(Duration::days(5));

        let duration = compute_duration_days(&task);
        assert_eq!(duration, 5);
    }
}
