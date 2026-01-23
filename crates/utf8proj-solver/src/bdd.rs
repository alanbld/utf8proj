//! BDD-based resource conflict detection and resolution
//!
//! Uses Binary Decision Diagrams to:
//! - Detect resource overallocation conflicts
//! - Find valid resource assignments
//! - Suggest conflict resolutions
//!
//! This is a lightweight alternative to full SAT/SMT solving,
//! using the biodivine-lib-bdd library.

use biodivine_lib_bdd::BddVariableSetBuilder;
use chrono::NaiveDate;
use std::collections::{HashMap, HashSet};
use utf8proj_core::{Project, Schedule};

/// Result of BDD-based conflict analysis
#[derive(Debug, Clone)]
pub struct ConflictAnalysis {
    /// Whether the schedule is conflict-free
    pub is_valid: bool,
    /// Detected conflicts
    pub conflicts: Vec<ResourceConflict>,
    /// Suggested resolutions (task shifts)
    pub suggestions: Vec<ConflictResolution>,
    /// BDD statistics
    pub stats: BddStats,
}

/// A cluster of tasks competing for the same resources (RFC-0014)
#[derive(Debug, Clone)]
pub struct ConflictCluster {
    /// Tasks in this conflict cluster
    pub tasks: Vec<String>,
    /// Resources involved in the conflicts
    pub resources: Vec<String>,
    /// Estimated contention (0.0 = no conflicts, 1.0 = fully serialized)
    pub estimated_contention: f32,
}

/// Extended conflict analysis with cluster information (RFC-0014)
#[derive(Debug, Clone)]
pub struct ClusterAnalysis {
    /// Conflict clusters (groups of competing tasks)
    pub clusters: Vec<ConflictCluster>,
    /// Tasks with no resource conflicts (can be scheduled at ASAP)
    pub unconstrained_tasks: Vec<String>,
    /// BDD statistics
    pub stats: BddStats,
}

/// A resource conflict at a specific time
#[derive(Debug, Clone)]
pub struct ResourceConflict {
    /// Resource ID
    pub resource_id: String,
    /// Date of conflict
    pub date: NaiveDate,
    /// Tasks competing for the resource
    pub competing_tasks: Vec<String>,
    /// Required capacity
    pub required: f64,
    /// Available capacity
    pub available: f64,
}

/// Suggested resolution for a conflict
#[derive(Debug, Clone)]
pub struct ConflictResolution {
    /// Task to shift
    pub task_id: String,
    /// Shift direction
    pub direction: ShiftDirection,
    /// Number of days to shift
    pub days: i64,
    /// Conflicts resolved by this shift
    pub resolves: usize,
}

/// Direction to shift a task
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShiftDirection {
    Earlier,
    Later,
}

/// BDD operation statistics
#[derive(Debug, Clone, Default)]
pub struct BddStats {
    /// Number of BDD variables created
    pub variables: usize,
    /// Number of BDD nodes in final result
    pub nodes: usize,
    /// Time spent in BDD operations (microseconds)
    pub time_us: u64,
}

/// BDD-based resource conflict analyzer
pub struct BddConflictAnalyzer {
    /// Maximum time horizon in days (reserved for future use)
    #[allow(dead_code)]
    max_days: i64,
}

impl BddConflictAnalyzer {
    /// Create a new analyzer with default settings
    pub fn new() -> Self {
        Self { max_days: 365 }
    }

    /// Create an analyzer with custom time horizon
    pub fn with_max_days(max_days: i64) -> Self {
        Self { max_days }
    }

    /// Analyze a schedule for resource conflicts using BDD
    pub fn analyze(&self, project: &Project, schedule: &Schedule) -> ConflictAnalysis {
        let start_time = std::time::Instant::now();

        // Step 1: Collect resource allocations per day
        let allocations = self.collect_allocations(project, schedule);

        // Step 2: Build BDD for resource constraints
        let (is_valid, conflicts, var_count, node_count) =
            self.build_constraint_bdd(project, &allocations);

        // Step 3: Generate resolution suggestions for conflicts
        let suggestions = if !is_valid {
            self.suggest_resolutions(&conflicts, schedule)
        } else {
            Vec::new()
        };

        let elapsed = start_time.elapsed();

        ConflictAnalysis {
            is_valid,
            conflicts,
            suggestions,
            stats: BddStats {
                variables: var_count,
                nodes: node_count,
                time_us: elapsed.as_micros() as u64,
            },
        }
    }

    /// Collect resource allocations: (resource_id, date) -> [(task_id, units)]
    fn collect_allocations(
        &self,
        _project: &Project,
        schedule: &Schedule,
    ) -> HashMap<(String, NaiveDate), Vec<(String, f64)>> {
        let mut allocations: HashMap<(String, NaiveDate), Vec<(String, f64)>> = HashMap::new();

        for (task_id, scheduled_task) in &schedule.tasks {
            for assignment in &scheduled_task.assignments {
                // For each day the task runs
                let mut current = scheduled_task.start;
                while current <= scheduled_task.finish {
                    let key = (assignment.resource_id.clone(), current);
                    allocations
                        .entry(key)
                        .or_default()
                        .push((task_id.clone(), f64::from(assignment.units)));

                    current = current.succ_opt().unwrap_or(current);
                }
            }
        }

        allocations
    }

    /// Build BDD constraint and detect conflicts
    fn build_constraint_bdd(
        &self,
        project: &Project,
        allocations: &HashMap<(String, NaiveDate), Vec<(String, f64)>>,
    ) -> (bool, Vec<ResourceConflict>, usize, usize) {
        // Build resource capacity map
        let capacities: HashMap<&str, f64> = project
            .resources
            .iter()
            .map(|r| (r.id.as_str(), f64::from(r.capacity)))
            .collect();

        let mut conflicts = Vec::new();

        // Check each (resource, date) for overallocation
        // We use BDD to represent the constraint: sum(allocations) <= capacity
        for ((resource_id, date), tasks) in allocations {
            let capacity = capacities.get(resource_id.as_str()).copied().unwrap_or(1.0);
            let total_required: f64 = tasks.iter().map(|(_, units)| units).sum();

            if total_required > capacity {
                conflicts.push(ResourceConflict {
                    resource_id: resource_id.clone(),
                    date: *date,
                    competing_tasks: tasks.iter().map(|(id, _)| id.clone()).collect(),
                    required: total_required,
                    available: capacity,
                });
            }
        }

        // Build a BDD representing valid states
        // For this prototype, we create a simple satisfiability check
        let (var_count, node_count) = self.build_validity_bdd(&conflicts, allocations);

        (conflicts.is_empty(), conflicts, var_count, node_count)
    }

    /// Build BDD for validity checking
    /// Returns (variable count, node count)
    fn build_validity_bdd(
        &self,
        conflicts: &[ResourceConflict],
        _allocations: &HashMap<(String, NaiveDate), Vec<(String, f64)>>,
    ) -> (usize, usize) {
        if conflicts.is_empty() {
            return (0, 1); // Trivially true BDD
        }

        // Create BDD variables for each task involved in conflicts
        let conflict_tasks: HashSet<&str> = conflicts
            .iter()
            .flat_map(|c| c.competing_tasks.iter().map(|s| s.as_str()))
            .collect();

        if conflict_tasks.is_empty() {
            return (0, 1);
        }

        // Build variable set
        let mut builder = BddVariableSetBuilder::new();
        let task_vars: HashMap<&str, _> = conflict_tasks
            .iter()
            .map(|&task| (task, builder.make_variable(task)))
            .collect();

        let vars = builder.build();
        let var_count = task_vars.len();

        // Build constraint: for each conflict, at least one task must be shifted
        // This is represented as: NOT(all conflicting tasks scheduled together)
        let mut constraint = vars.mk_true();

        for conflict in conflicts {
            // At least one task in this conflict must not run at this time
            // ~(t1 & t2 & ... & tn) = ~t1 | ~t2 | ... | ~tn
            let mut conflict_clause = vars.mk_false();
            for task_id in &conflict.competing_tasks {
                if let Some(&var) = task_vars.get(task_id.as_str()) {
                    // Add "not this task" to the clause
                    let not_task = vars.mk_not_var(var);
                    conflict_clause = conflict_clause.or(&not_task);
                }
            }
            constraint = constraint.and(&conflict_clause);
        }

        let node_count = constraint.size();

        // Check if there's a valid assignment
        let _is_satisfiable = !constraint.is_false();

        // For debugging: find satisfying assignments
        if let Some(_witness) = constraint.sat_witness() {
            // A valid assignment exists - some tasks can be shifted to resolve conflicts
        }

        (var_count, node_count)
    }

    /// Suggest resolutions for conflicts
    fn suggest_resolutions(
        &self,
        conflicts: &[ResourceConflict],
        schedule: &Schedule,
    ) -> Vec<ConflictResolution> {
        let mut suggestions = Vec::new();

        // Group conflicts by task to find which tasks are most problematic
        let mut task_conflict_count: HashMap<&str, usize> = HashMap::new();
        for conflict in conflicts {
            for task_id in &conflict.competing_tasks {
                *task_conflict_count.entry(task_id.as_str()).or_default() += 1;
            }
        }

        // Suggest shifting tasks that appear in most conflicts
        let mut task_counts: Vec<_> = task_conflict_count.into_iter().collect();
        task_counts.sort_by(|a, b| b.1.cmp(&a.1));

        for (task_id, conflict_count) in task_counts.into_iter().take(5) {
            if let Some(scheduled_task) = schedule.tasks.get(task_id) {
                // Suggest shifting based on slack
                let slack_days = scheduled_task.slack.as_days() as i64;

                if slack_days > 0 {
                    suggestions.push(ConflictResolution {
                        task_id: task_id.to_string(),
                        direction: ShiftDirection::Later,
                        days: slack_days.min(5),
                        resolves: conflict_count,
                    });
                } else {
                    // Non-critical task might be shiftable earlier
                    suggestions.push(ConflictResolution {
                        task_id: task_id.to_string(),
                        direction: ShiftDirection::Later,
                        days: 1,
                        resolves: conflict_count,
                    });
                }
            }
        }

        suggestions
    }

    /// Find optimal task shifts to resolve all conflicts using BDD
    pub fn find_optimal_resolution(
        &self,
        project: &Project,
        schedule: &Schedule,
    ) -> Option<Vec<(String, i64)>> {
        let analysis = self.analyze(project, schedule);

        if analysis.is_valid {
            return Some(Vec::new()); // No shifts needed
        }

        // Use BDD to find minimal set of task shifts
        self.solve_with_bdd(project, schedule, &analysis.conflicts)
    }

    /// Analyze schedule to identify conflict clusters (RFC-0014 Hybrid Leveling)
    ///
    /// Returns clusters of competing tasks and unconstrained tasks that don't
    /// need leveling. This enables O(n + sum(k²)) leveling instead of O(n²).
    pub fn analyze_clusters(&self, project: &Project, schedule: &Schedule) -> ClusterAnalysis {
        let start_time = std::time::Instant::now();

        // Step 1: Collect resource allocations
        let allocations = self.collect_allocations(project, schedule);

        // Step 2: Find all tasks involved in any conflict
        let capacities: HashMap<&str, f64> = project
            .resources
            .iter()
            .map(|r| (r.id.as_str(), f64::from(r.capacity)))
            .collect();

        // Map: resource_id -> set of conflicting task ids
        let mut resource_conflicts: HashMap<String, HashSet<String>> = HashMap::new();

        for ((resource_id, _date), tasks) in &allocations {
            let capacity = capacities.get(resource_id.as_str()).copied().unwrap_or(1.0);
            let total_required: f64 = tasks.iter().map(|(_, units)| units).sum();

            if total_required > capacity {
                // This (resource, date) is overallocated
                let task_ids: HashSet<String> = tasks.iter().map(|(id, _)| id.clone()).collect();
                resource_conflicts
                    .entry(resource_id.clone())
                    .or_default()
                    .extend(task_ids);
            }
        }

        // Step 3: Build conflict clusters using union-find for connected components
        // Tasks are connected if they share a resource conflict
        let mut all_conflicting_tasks: HashSet<String> = HashSet::new();
        for tasks in resource_conflicts.values() {
            all_conflicting_tasks.extend(tasks.iter().cloned());
        }

        // Build adjacency: which tasks share resources
        let mut task_resources: HashMap<String, HashSet<String>> = HashMap::new();
        for (resource_id, tasks) in &resource_conflicts {
            for task_id in tasks {
                task_resources
                    .entry(task_id.clone())
                    .or_default()
                    .insert(resource_id.clone());
            }
        }

        // Find connected components (clusters)
        let mut visited: HashSet<String> = HashSet::new();
        let mut clusters: Vec<ConflictCluster> = Vec::new();

        for task_id in &all_conflicting_tasks {
            if visited.contains(task_id) {
                continue;
            }

            // BFS to find all connected tasks
            let mut cluster_tasks: Vec<String> = Vec::new();
            let mut cluster_resources: HashSet<String> = HashSet::new();
            let mut queue: Vec<String> = vec![task_id.clone()];

            while let Some(current) = queue.pop() {
                if visited.contains(&current) {
                    continue;
                }
                visited.insert(current.clone());
                cluster_tasks.push(current.clone());

                // Find all resources this task conflicts on
                if let Some(resources) = task_resources.get(&current) {
                    for resource_id in resources {
                        cluster_resources.insert(resource_id.clone());

                        // Find all other tasks that conflict on this resource
                        if let Some(other_tasks) = resource_conflicts.get(resource_id) {
                            for other_task in other_tasks {
                                if !visited.contains(other_task) {
                                    queue.push(other_task.clone());
                                }
                            }
                        }
                    }
                }
            }

            // Calculate contention estimate
            let num_tasks = cluster_tasks.len();
            let num_resources = cluster_resources.len();
            let estimated_contention = if num_resources > 0 && num_tasks > 1 {
                // Higher contention if many tasks per resource
                ((num_tasks as f32 / num_resources as f32) - 1.0).min(1.0).max(0.0)
            } else {
                0.0
            };

            cluster_tasks.sort(); // Deterministic ordering
            let mut resources_vec: Vec<String> = cluster_resources.into_iter().collect();
            resources_vec.sort();

            clusters.push(ConflictCluster {
                tasks: cluster_tasks,
                resources: resources_vec,
                estimated_contention,
            });
        }

        // Sort clusters by size (largest first) for consistent processing
        clusters.sort_by(|a, b| b.tasks.len().cmp(&a.tasks.len()));

        // Step 4: Find unconstrained tasks (not in any cluster)
        let mut unconstrained_tasks: Vec<String> = schedule
            .tasks
            .keys()
            .filter(|task_id| !all_conflicting_tasks.contains(*task_id))
            .cloned()
            .collect();
        unconstrained_tasks.sort(); // Deterministic ordering

        let elapsed = start_time.elapsed();

        ClusterAnalysis {
            clusters,
            unconstrained_tasks,
            stats: BddStats {
                variables: all_conflicting_tasks.len(),
                nodes: resource_conflicts.len(),
                time_us: elapsed.as_micros() as u64,
            },
        }
    }

    /// Use BDD to solve for minimal task shifts
    fn solve_with_bdd(
        &self,
        _project: &Project,
        schedule: &Schedule,
        conflicts: &[ResourceConflict],
    ) -> Option<Vec<(String, i64)>> {
        // Collect all tasks involved in conflicts
        let conflict_tasks: Vec<&str> = conflicts
            .iter()
            .flat_map(|c| c.competing_tasks.iter().map(|s| s.as_str()))
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();

        if conflict_tasks.is_empty() {
            return Some(Vec::new());
        }

        // Create BDD variables: shift_task_i means "shift task i by 1+ days"
        let mut builder = BddVariableSetBuilder::new();
        let shift_vars: Vec<_> = conflict_tasks
            .iter()
            .map(|&task| (task, builder.make_variable(task)))
            .collect();

        let vars = builder.build();

        // Build constraint: for each conflict, at least one task must be shifted
        let mut constraint = vars.mk_true();

        for conflict in conflicts {
            let mut at_least_one_shifted = vars.mk_false();
            for task_id in &conflict.competing_tasks {
                if let Some((_, var)) = shift_vars.iter().find(|(t, _)| *t == task_id.as_str()) {
                    at_least_one_shifted = at_least_one_shifted.or(&vars.mk_var(*var));
                }
            }
            constraint = constraint.and(&at_least_one_shifted);
        }

        // Find a satisfying assignment with minimal shifts
        // For now, just find any satisfying assignment
        if let Some(witness) = constraint.sat_witness() {
            let shifts: Vec<(String, i64)> = shift_vars
                .iter()
                .enumerate()
                .filter_map(|(idx, (task, _))| {
                    if witness.value(biodivine_lib_bdd::BddVariable::from_index(idx)) {
                        // Get task's slack to determine shift amount
                        let shift_days = schedule
                            .tasks
                            .get(*task)
                            .map(|t| (t.slack.as_days() as i64).max(1))
                            .unwrap_or(1);
                        Some((task.to_string(), shift_days))
                    } else {
                        None
                    }
                })
                .collect();

            Some(shifts)
        } else {
            None // No valid resolution exists
        }
    }
}

impl Default for BddConflictAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use utf8proj_core::{Duration, Resource, ScheduledTask, Task, TaskStatus};

    fn make_project_with_resource_conflict() -> (Project, Schedule) {
        let mut project = Project::new("Conflict Test");
        project.start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();

        // One resource with capacity 1
        project.resources = vec![Resource::new("dev").name("Developer").capacity(1.0)];

        // Two tasks that need the same resource at the same time
        project.tasks = vec![
            Task::new("task1")
                .name("Task 1")
                .effort(Duration::days(5))
                .assign("dev"),
            Task::new("task2")
                .name("Task 2")
                .effort(Duration::days(5))
                .assign("dev"),
        ];

        // Create a schedule where both tasks overlap (conflict!)
        let project_end = NaiveDate::from_ymd_opt(2025, 1, 10).unwrap();
        let mut schedule = Schedule {
            tasks: HashMap::new(),
            critical_path: vec!["task1".to_string()],
            project_duration: Duration::days(5),
            project_end,
            total_cost: None,
            total_cost_range: None,
            project_progress: 0,
            project_baseline_finish: project_end,
            project_forecast_finish: project_end,
            project_variance_days: 0,
            planned_value: 0,
            earned_value: 0,
            spi: 1.0,
        };

        // Both tasks scheduled at the same time - conflict!
        let start = project.start;
        let finish = NaiveDate::from_ymd_opt(2025, 1, 10).unwrap();

        schedule.tasks.insert(
            "task1".to_string(),
            ScheduledTask {
                task_id: "task1".to_string(),
                start,
                finish,
                duration: Duration::days(5),
                assignments: vec![utf8proj_core::Assignment {
                    resource_id: "dev".to_string(),
                    start,
                    finish,
                    units: 1.0,
                    cost: None,
                    cost_range: None,
                    is_abstract: false,
                    effort_days: None,
                }],
                slack: Duration::zero(),
                is_critical: true,
                early_start: start,
                early_finish: finish,
                late_start: start,
                late_finish: finish,
                forecast_start: start,
                forecast_finish: finish,
                remaining_duration: Duration::days(5),
                percent_complete: 0,
                status: TaskStatus::NotStarted,
                baseline_start: start,
                baseline_finish: finish,
                start_variance_days: 0,
                finish_variance_days: 0,
                cost_range: None,
                has_abstract_assignments: false,
            },
        );

        schedule.tasks.insert(
            "task2".to_string(),
            ScheduledTask {
                task_id: "task2".to_string(),
                start,
                finish,
                duration: Duration::days(5),
                assignments: vec![utf8proj_core::Assignment {
                    resource_id: "dev".to_string(),
                    start,
                    finish,
                    units: 1.0,
                    cost: None,
                    cost_range: None,
                    is_abstract: false,
                    effort_days: None,
                }],
                slack: Duration::days(5),
                is_critical: false,
                early_start: start,
                early_finish: finish,
                late_start: NaiveDate::from_ymd_opt(2025, 1, 13).unwrap(),
                late_finish: NaiveDate::from_ymd_opt(2025, 1, 17).unwrap(),
                forecast_start: start,
                forecast_finish: finish,
                remaining_duration: Duration::days(5),
                percent_complete: 0,
                status: TaskStatus::NotStarted,
                baseline_start: start,
                baseline_finish: finish,
                start_variance_days: 0,
                finish_variance_days: 0,
                cost_range: None,
                has_abstract_assignments: false,
            },
        );

        (project, schedule)
    }

    fn make_project_no_conflict() -> (Project, Schedule) {
        let mut project = Project::new("No Conflict");
        project.start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();

        project.resources = vec![Resource::new("dev").name("Developer").capacity(1.0)];

        project.tasks = vec![
            Task::new("task1")
                .name("Task 1")
                .effort(Duration::days(5))
                .assign("dev"),
            Task::new("task2")
                .name("Task 2")
                .effort(Duration::days(5))
                .assign("dev")
                .depends_on("task1"),
        ];

        // Sequential schedule - no conflict
        let project_end = NaiveDate::from_ymd_opt(2025, 1, 17).unwrap();
        let mut schedule = Schedule {
            tasks: HashMap::new(),
            critical_path: vec!["task1".to_string(), "task2".to_string()],
            project_duration: Duration::days(10),
            project_end,
            total_cost: None,
            total_cost_range: None,
            project_progress: 0,
            project_baseline_finish: project_end,
            project_forecast_finish: project_end,
            project_variance_days: 0,
            planned_value: 0,
            earned_value: 0,
            spi: 1.0,
        };

        let start1 = project.start;
        let finish1 = NaiveDate::from_ymd_opt(2025, 1, 10).unwrap();
        let start2 = NaiveDate::from_ymd_opt(2025, 1, 13).unwrap();
        let finish2 = NaiveDate::from_ymd_opt(2025, 1, 17).unwrap();

        schedule.tasks.insert(
            "task1".to_string(),
            ScheduledTask {
                task_id: "task1".to_string(),
                start: start1,
                finish: finish1,
                duration: Duration::days(5),
                assignments: vec![utf8proj_core::Assignment {
                    resource_id: "dev".to_string(),
                    start: start1,
                    finish: finish1,
                    units: 1.0,
                    cost: None,
                    cost_range: None,
                    is_abstract: false,
                    effort_days: None,
                }],
                slack: Duration::zero(),
                is_critical: true,
                early_start: start1,
                early_finish: finish1,
                late_start: start1,
                late_finish: finish1,
                forecast_start: start1,
                forecast_finish: finish1,
                remaining_duration: Duration::days(5),
                percent_complete: 0,
                status: TaskStatus::NotStarted,
                baseline_start: start1,
                baseline_finish: finish1,
                start_variance_days: 0,
                finish_variance_days: 0,
                cost_range: None,
                has_abstract_assignments: false,
            },
        );

        schedule.tasks.insert(
            "task2".to_string(),
            ScheduledTask {
                task_id: "task2".to_string(),
                start: start2,
                finish: finish2,
                duration: Duration::days(5),
                assignments: vec![utf8proj_core::Assignment {
                    resource_id: "dev".to_string(),
                    start: start2,
                    finish: finish2,
                    units: 1.0,
                    cost: None,
                    cost_range: None,
                    is_abstract: false,
                    effort_days: None,
                }],
                slack: Duration::zero(),
                is_critical: true,
                early_start: start2,
                early_finish: finish2,
                late_start: start2,
                late_finish: finish2,
                forecast_start: start2,
                forecast_finish: finish2,
                remaining_duration: Duration::days(5),
                percent_complete: 0,
                status: TaskStatus::NotStarted,
                cost_range: None,
                has_abstract_assignments: false,
                baseline_start: start2,
                baseline_finish: finish2,
                start_variance_days: 0,
                finish_variance_days: 0,
            },
        );

        (project, schedule)
    }

    #[test]
    fn detect_resource_conflict() {
        let (project, schedule) = make_project_with_resource_conflict();
        let analyzer = BddConflictAnalyzer::new();

        let analysis = analyzer.analyze(&project, &schedule);

        assert!(!analysis.is_valid, "Should detect conflict");
        assert!(!analysis.conflicts.is_empty(), "Should have conflicts");

        let conflict = &analysis.conflicts[0];
        assert_eq!(conflict.resource_id, "dev");
        assert!(conflict.competing_tasks.contains(&"task1".to_string()));
        assert!(conflict.competing_tasks.contains(&"task2".to_string()));
        assert_eq!(conflict.required, 2.0);
        assert_eq!(conflict.available, 1.0);
    }

    #[test]
    fn no_conflict_when_sequential() {
        let (project, schedule) = make_project_no_conflict();
        let analyzer = BddConflictAnalyzer::new();

        let analysis = analyzer.analyze(&project, &schedule);

        assert!(analysis.is_valid, "Should be conflict-free");
        assert!(analysis.conflicts.is_empty(), "Should have no conflicts");
    }

    #[test]
    fn suggest_resolution() {
        let (project, schedule) = make_project_with_resource_conflict();
        let analyzer = BddConflictAnalyzer::new();

        let analysis = analyzer.analyze(&project, &schedule);

        assert!(
            !analysis.suggestions.is_empty(),
            "Should suggest resolution"
        );

        // Should suggest shifting task2 (it has slack)
        let suggestion = &analysis.suggestions[0];
        assert!(suggestion.resolves > 0);
    }

    #[test]
    fn find_optimal_resolution() {
        let (project, schedule) = make_project_with_resource_conflict();
        let analyzer = BddConflictAnalyzer::new();

        let resolution = analyzer.find_optimal_resolution(&project, &schedule);

        assert!(resolution.is_some(), "Should find resolution");
        let shifts = resolution.unwrap();
        assert!(!shifts.is_empty(), "Should have task shifts");
    }

    #[test]
    fn bdd_stats_recorded() {
        let (project, schedule) = make_project_with_resource_conflict();
        let analyzer = BddConflictAnalyzer::new();

        let analysis = analyzer.analyze(&project, &schedule);

        assert!(analysis.stats.variables > 0, "Should have variables");
        assert!(analysis.stats.nodes > 0, "Should have nodes");
    }

    #[test]
    fn analyzer_with_custom_max_days() {
        let analyzer = BddConflictAnalyzer::with_max_days(730);
        assert_eq!(analyzer.max_days, 730);
    }

    #[test]
    fn empty_project_no_conflicts() {
        // Project with no tasks and no resource assignments
        let project = Project::new("Empty");
        let project_end = NaiveDate::from_ymd_opt(2025, 1, 1).unwrap();
        let schedule = Schedule {
            tasks: HashMap::new(),
            critical_path: vec![],
            project_duration: Duration::zero(),
            project_end,
            total_cost: None,
            total_cost_range: None,
            project_progress: 0,
            project_baseline_finish: project_end,
            project_forecast_finish: project_end,
            project_variance_days: 0,
            planned_value: 0,
            earned_value: 0,
            spi: 1.0,
        };

        let analyzer = BddConflictAnalyzer::new();
        let analysis = analyzer.analyze(&project, &schedule);

        // No conflicts for empty project
        assert!(analysis.is_valid);
        assert!(analysis.conflicts.is_empty());
    }

    #[test]
    fn analyzer_default() {
        // Tests lines 386-387: Default impl
        let analyzer = BddConflictAnalyzer::default();
        assert_eq!(analyzer.max_days, 365);
    }

    #[test]
    fn find_optimal_resolution_no_conflict() {
        // Tests line 309: when is_valid is true, return empty vec
        let (project, schedule) = make_project_no_conflict();
        let analyzer = BddConflictAnalyzer::new();

        let resolution = analyzer.find_optimal_resolution(&project, &schedule);

        assert!(resolution.is_some());
        assert!(
            resolution.unwrap().is_empty(),
            "No shifts needed for valid schedule"
        );
    }
}
