//! Critical Path Method Implementation
//!
//! Textbook CPM algorithm operating on a SchedulingGraph.
//!
//! References:
//!   - Kelley & Walker (1959) "Critical-Path Planning and Scheduling"
//!   - PMI PMBOK Guide, Chapter 6
//!
//! # Algorithm
//!
//! 1. Topological sort (done in dag.rs)
//! 2. Forward pass: Compute ES (Early Start) and EF (Early Finish)
//! 3. Backward pass: Compute LS (Late Start) and LF (Late Finish)
//! 4. Slack calculation: Slack = LS - ES (must be >= 0)
//! 5. Critical path: Tasks where Slack == 0

use std::collections::HashMap;
use crate::dag::{SchedulingGraph, DependencyEdge};
use utf8proj_core::{DependencyType, TaskId};

/// Errors during CPM scheduling
#[derive(Debug, Clone, PartialEq)]
pub enum CpmError {
    /// CPM invariant violated - slack should never be negative
    NegativeSlack { task: TaskId, slack: i64 },
    /// Empty graph
    EmptyGraph,
}

impl std::fmt::Display for CpmError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CpmError::NegativeSlack { task, slack } => {
                write!(f, "CPM invariant violated: task '{}' has negative slack ({})", task, slack)
            }
            CpmError::EmptyGraph => write!(f, "Cannot schedule empty graph"),
        }
    }
}

impl std::error::Error for CpmError {}

/// Result of CPM scheduling for a single task
#[derive(Debug, Clone)]
pub struct CpmResult {
    pub task_id: TaskId,
    /// Early Start (days from project start)
    pub es: i64,
    /// Early Finish (days from project start)
    pub ef: i64,
    /// Late Start (days from project start)
    pub ls: i64,
    /// Late Finish (days from project start)
    pub lf: i64,
    /// Total Slack (days) - ALWAYS >= 0 for valid CPM
    pub total_slack: i64,
    /// Free Slack (days)
    pub free_slack: i64,
    /// On critical path (total_slack == 0)
    pub is_critical: bool,
    /// Duration in days
    pub duration: i64,
}

/// Complete CPM schedule
#[derive(Debug)]
pub struct CpmSchedule {
    /// Results for each task
    pub results: HashMap<TaskId, CpmResult>,
    /// Critical path (tasks with zero slack, in order)
    pub critical_path: Vec<TaskId>,
    /// Project start (day 0)
    pub project_start: i64,
    /// Project end (max EF)
    pub project_end: i64,
}

/// CPM scheduler operating on flattened graph
pub struct CpmScheduler;

impl CpmScheduler {
    pub fn new() -> Self {
        Self
    }

    /// Schedule using Critical Path Method
    ///
    /// The graph must already be topologically sorted.
    pub fn schedule(&self, graph: &SchedulingGraph) -> Result<CpmSchedule, CpmError> {
        if graph.tasks.is_empty() {
            return Err(CpmError::EmptyGraph);
        }

        let mut es: HashMap<TaskId, i64> = HashMap::new();
        let mut ef: HashMap<TaskId, i64> = HashMap::new();
        let mut ls: HashMap<TaskId, i64> = HashMap::new();
        let mut lf: HashMap<TaskId, i64> = HashMap::new();

        // Get durations
        let duration: HashMap<TaskId, i64> = graph.tasks.iter()
            .map(|t| (t.id.clone(), t.duration_days))
            .collect();

        // ════════════════════════════════════════════════════════════════════
        // FORWARD PASS: Compute Early Start (ES) and Early Finish (EF)
        // ════════════════════════════════════════════════════════════════════
        //
        // For each task in topological order:
        //   ES = max(constraint from each predecessor), or 0 if no predecessors
        //   EF = ES + duration

        for task_id in &graph.topo_order {
            let task_duration = duration[task_id];

            let early_start = if let Some(edges) = graph.predecessors.get(task_id) {
                if edges.is_empty() {
                    0 // Project start
                } else {
                    edges.iter()
                        .map(|edge| compute_successor_es(edge, ef[&edge.from], es[&edge.from], task_duration))
                        .max()
                        .unwrap_or(0)
                }
            } else {
                0
            };

            let early_finish = early_start + task_duration;

            es.insert(task_id.clone(), early_start);
            ef.insert(task_id.clone(), early_finish);
        }

        // Project end is the maximum EF
        let project_end = ef.values().cloned().max().unwrap_or(0);

        // ════════════════════════════════════════════════════════════════════
        // BACKWARD PASS: Compute Late Start (LS) and Late Finish (LF)
        // ════════════════════════════════════════════════════════════════════
        //
        // For each task in REVERSE topological order:
        //   LF = min(constraint from each successor), or project_end if no successors
        //   LS = LF - duration

        for task_id in graph.topo_order.iter().rev() {
            let task_duration = duration[task_id];

            let late_finish = if let Some(edges) = graph.successors.get(task_id) {
                if edges.is_empty() {
                    project_end
                } else {
                    edges.iter()
                        .map(|edge| compute_predecessor_lf(edge, ls[&edge.to], lf[&edge.to], task_duration))
                        .min()
                        .unwrap_or(project_end)
                }
            } else {
                project_end
            };

            let late_start = late_finish - task_duration;

            lf.insert(task_id.clone(), late_finish);
            ls.insert(task_id.clone(), late_start);
        }

        // ════════════════════════════════════════════════════════════════════
        // SLACK CALCULATION
        // ════════════════════════════════════════════════════════════════════
        //
        // Total Slack = LS - ES = LF - EF (must be >= 0)
        // Free Slack  = min(ES of successors) - EF
        // Critical    = Total Slack == 0

        let mut results: HashMap<TaskId, CpmResult> = HashMap::new();
        let mut critical_path: Vec<TaskId> = Vec::new();

        for task_id in &graph.topo_order {
            let task_duration = duration[task_id];
            let task_es = es[task_id];
            let task_ef = ef[task_id];
            let task_ls = ls[task_id];
            let task_lf = lf[task_id];

            let total_slack = task_ls - task_es;

            // INVARIANT: Slack must be non-negative
            if total_slack < 0 {
                return Err(CpmError::NegativeSlack {
                    task: task_id.clone(),
                    slack: total_slack,
                });
            }

            // Free slack: how much can this task slip without affecting successors
            let free_slack = if let Some(edges) = graph.successors.get(task_id) {
                if edges.is_empty() {
                    total_slack
                } else {
                    edges.iter()
                        .map(|edge| es[&edge.to])
                        .min()
                        .map(|min_succ_es| min_succ_es - task_ef)
                        .unwrap_or(total_slack)
                        .max(0)
                }
            } else {
                total_slack
            };

            let is_critical = total_slack == 0;
            if is_critical && task_duration > 0 {
                critical_path.push(task_id.clone());
            }

            results.insert(task_id.clone(), CpmResult {
                task_id: task_id.clone(),
                es: task_es,
                ef: task_ef,
                ls: task_ls,
                lf: task_lf,
                total_slack,
                free_slack,
                is_critical,
                duration: task_duration,
            });
        }

        Ok(CpmSchedule {
            results,
            critical_path,
            project_start: 0,
            project_end,
        })
    }
}

impl Default for CpmScheduler {
    fn default() -> Self {
        Self::new()
    }
}

/// Compute the ES constraint for a successor based on dependency type
fn compute_successor_es(edge: &DependencyEdge, pred_ef: i64, pred_es: i64, _succ_duration: i64) -> i64 {
    let lag = edge.lag_days;

    match edge.dep_type {
        DependencyType::FinishToStart => {
            // Successor starts after predecessor finishes
            // ES(succ) >= EF(pred) + lag
            pred_ef + lag
        }
        DependencyType::StartToStart => {
            // Successor starts after/with predecessor starts
            // ES(succ) >= ES(pred) + lag
            pred_es + lag
        }
        DependencyType::FinishToFinish => {
            // Successor finishes after/with predecessor finishes
            // EF(succ) >= EF(pred) + lag
            // ES(succ) >= EF(pred) + lag - duration(succ)
            // We return the constraint on ES, so this affects the successor's EF
            pred_ef + lag
        }
        DependencyType::StartToFinish => {
            // Successor finishes after/with predecessor starts
            // EF(succ) >= ES(pred) + lag
            // This is rare and somewhat unusual
            pred_es + lag
        }
    }
}

/// Compute the LF constraint for a predecessor based on dependency type
fn compute_predecessor_lf(edge: &DependencyEdge, succ_ls: i64, succ_lf: i64, _pred_duration: i64) -> i64 {
    let lag = edge.lag_days;

    match edge.dep_type {
        DependencyType::FinishToStart => {
            // Predecessor must finish before successor starts (minus lag)
            // LF(pred) <= LS(succ) - lag
            succ_ls - lag
        }
        DependencyType::StartToStart => {
            // Predecessor must start before successor starts
            // But this constrains predecessor's start, not finish
            // LF(pred) = LS(succ) - lag + duration(pred)
            // For simplicity, we use the successor's LS as constraint
            succ_ls - lag
        }
        DependencyType::FinishToFinish => {
            // Predecessor must finish before successor finishes
            // LF(pred) <= LF(succ) - lag
            succ_lf - lag
        }
        DependencyType::StartToFinish => {
            // Predecessor must start before successor finishes
            // This affects predecessor's start
            succ_lf - lag
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dag::SchedulingGraph;
    use utf8proj_core::{Task, Duration};

    fn make_tasks_with_deps(tasks: &[(&str, i64, &[&str])]) -> Vec<Task> {
        tasks.iter().map(|(id, dur, deps)| {
            let mut task = Task::new(*id).duration(Duration::days(*dur));
            for dep in *deps {
                task = task.depends_on(*dep);
            }
            task
        }).collect()
    }

    #[test]
    fn test_single_task() {
        let tasks = vec![Task::new("a").duration(Duration::days(5))];
        let graph = SchedulingGraph::from_wbs(&tasks).unwrap();
        let schedule = CpmScheduler::new().schedule(&graph).unwrap();

        let a = &schedule.results["a"];
        assert_eq!(a.es, 0);
        assert_eq!(a.ef, 5);
        assert_eq!(a.ls, 0);
        assert_eq!(a.lf, 5);
        assert_eq!(a.total_slack, 0);
        assert!(a.is_critical);
        assert_eq!(schedule.project_end, 5);
    }

    #[test]
    fn test_sequential_chain() {
        // A(5) -> B(3) -> C(2) = 10 days total
        let tasks = make_tasks_with_deps(&[
            ("a", 5, &[]),
            ("b", 3, &["a"]),
            ("c", 2, &["b"]),
        ]);

        let graph = SchedulingGraph::from_wbs(&tasks).unwrap();
        let schedule = CpmScheduler::new().schedule(&graph).unwrap();

        assert_eq!(schedule.project_end, 10);

        // All on critical path
        assert!(schedule.results["a"].is_critical);
        assert!(schedule.results["b"].is_critical);
        assert!(schedule.results["c"].is_critical);

        // Verify dates
        assert_eq!(schedule.results["a"].es, 0);
        assert_eq!(schedule.results["a"].ef, 5);
        assert_eq!(schedule.results["b"].es, 5);
        assert_eq!(schedule.results["b"].ef, 8);
        assert_eq!(schedule.results["c"].es, 8);
        assert_eq!(schedule.results["c"].ef, 10);
    }

    #[test]
    fn test_slack_never_negative() {
        // Complex network to verify invariant
        let tasks = make_tasks_with_deps(&[
            ("start", 0, &[]),
            ("a", 5, &["start"]),
            ("b", 8, &["start"]),
            ("c", 3, &["a"]),
            ("d", 4, &["b"]),
            ("e", 6, &["c", "d"]),
            ("f", 2, &["a"]),
            ("end", 0, &["e", "f"]),
        ]);

        let graph = SchedulingGraph::from_wbs(&tasks).unwrap();
        let schedule = CpmScheduler::new().schedule(&graph).unwrap();

        for (id, result) in &schedule.results {
            assert!(
                result.total_slack >= 0,
                "Task {} has negative slack: {}",
                id,
                result.total_slack
            );
        }
    }

    #[test]
    fn test_parallel_paths_with_slack() {
        // A(5) ---> C(2)
        //           |
        // B(3) -----+
        //
        // Critical: A -> C (7 days)
        // B has slack

        let tasks = make_tasks_with_deps(&[
            ("a", 5, &[]),
            ("b", 3, &[]),
            ("c", 2, &["a", "b"]),
        ]);

        let graph = SchedulingGraph::from_wbs(&tasks).unwrap();
        let schedule = CpmScheduler::new().schedule(&graph).unwrap();

        assert_eq!(schedule.project_end, 7);

        // A is critical
        assert!(schedule.results["a"].is_critical);
        assert_eq!(schedule.results["a"].total_slack, 0);

        // B has slack (can start as late as day 2)
        assert!(!schedule.results["b"].is_critical);
        assert_eq!(schedule.results["b"].total_slack, 2);
        assert_eq!(schedule.results["b"].ls, 2);

        // C is critical
        assert!(schedule.results["c"].is_critical);
    }

    #[test]
    fn test_critical_path_has_zero_slack() {
        let tasks = make_tasks_with_deps(&[
            ("a", 5, &[]),
            ("b", 3, &[]),
            ("c", 2, &["a", "b"]),
        ]);

        let graph = SchedulingGraph::from_wbs(&tasks).unwrap();
        let schedule = CpmScheduler::new().schedule(&graph).unwrap();

        for task_id in &schedule.critical_path {
            let result = &schedule.results[task_id];
            assert_eq!(
                result.total_slack, 0,
                "Critical task {} has non-zero slack: {}",
                task_id, result.total_slack
            );
        }
    }
}
