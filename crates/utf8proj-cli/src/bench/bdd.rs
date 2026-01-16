//! BDD-specific benchmarks
//!
//! Compares BDD conflict resolution vs heuristic leveling.

use chrono::NaiveDate;
use std::time::{Duration, Instant};

use utf8proj_core::{Calendar, Project, Resource, Scheduler, Task};
use utf8proj_solver::{level_resources, BddConflictAnalyzer, CpmSolver};

/// Result of a BDD benchmark run
#[derive(Debug)]
pub struct BddBenchmarkResult {
    pub scenario: String,
    pub task_count: usize,
    pub resource_count: usize,
    pub conflict_count: usize,
    pub schedule_time: Duration,
    pub bdd_analysis_time: Duration,
    pub heuristic_level_time: Duration,
    pub bdd_conflicts_found: usize,
    pub bdd_resolution_found: bool,
    pub heuristic_conflicts_remaining: usize,
    pub bdd_nodes: usize,
    pub status: BddBenchmarkStatus,
}

/// Status of a BDD benchmark
#[derive(Debug)]
pub enum BddBenchmarkStatus {
    Success,
    Error(String),
}

impl std::fmt::Display for BddBenchmarkStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BddBenchmarkStatus::Success => write!(f, "OK"),
            BddBenchmarkStatus::Error(e) => write!(f, "ERROR: {}", e),
        }
    }
}

/// Scenario types for BDD benchmarks
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum BddScenario {
    /// All tasks compete for single resource
    SingleResource,
    /// Tasks distributed across multiple resources with some conflicts
    MultiResource,
    /// Complex web of resource dependencies
    ResourceWeb,
}

impl std::fmt::Display for BddScenario {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BddScenario::SingleResource => write!(f, "SingleRes"),
            BddScenario::MultiResource => write!(f, "MultiRes"),
            BddScenario::ResourceWeb => write!(f, "ResWeb"),
        }
    }
}

/// Generate a project with resource conflicts
pub fn generate_conflict_project(
    scenario: BddScenario,
    task_count: usize,
    resource_count: usize,
) -> Project {
    match scenario {
        BddScenario::SingleResource => generate_single_resource_conflict(task_count),
        BddScenario::MultiResource => generate_multi_resource_conflict(task_count, resource_count),
        BddScenario::ResourceWeb => generate_resource_web(task_count, resource_count),
    }
}

/// All tasks compete for a single resource (maximum conflict)
fn generate_single_resource_conflict(task_count: usize) -> Project {
    let mut project = Project::new("Single Resource Conflict");
    project.start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
    project.calendars = vec![Calendar::default()];

    // Single resource with capacity 1
    project.resources = vec![Resource::new("dev").name("Developer").capacity(1.0)];

    // All tasks need the same resource, no dependencies (all parallel = maximum conflict)
    for i in 0..task_count {
        let task = Task::new(&format!("task_{:04}", i))
            .name(format!("Task {}", i))
            .duration(utf8proj_core::Duration::days(5))
            .assign("dev");
        project.tasks.push(task);
    }

    project
}

/// Tasks distributed across resources with partial conflicts
fn generate_multi_resource_conflict(task_count: usize, resource_count: usize) -> Project {
    let mut project = Project::new("Multi Resource Conflict");
    project.start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
    project.calendars = vec![Calendar::default()];

    // Create resources with capacity 2 each
    project.resources = (0..resource_count)
        .map(|i| {
            Resource::new(&format!("res_{}", i))
                .name(format!("Resource {}", i))
                .capacity(2.0)
        })
        .collect();

    // Distribute tasks across resources (3 tasks per resource = 1 conflict each)
    for i in 0..task_count {
        let res_idx = i % resource_count;
        let task = Task::new(&format!("task_{:04}", i))
            .name(format!("Task {}", i))
            .duration(utf8proj_core::Duration::days(3))
            .assign(&format!("res_{}", res_idx));
        project.tasks.push(task);
    }

    project
}

/// Complex resource dependencies with varying conflict density
fn generate_resource_web(task_count: usize, resource_count: usize) -> Project {
    let mut project = Project::new("Resource Web");
    project.start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
    project.calendars = vec![Calendar::default()];

    // Resources with varying capacities
    project.resources = (0..resource_count)
        .map(|i| {
            let capacity = 1.0 + (i % 3) as f32; // Capacities: 1, 2, 3, 1, 2, 3...
            Resource::new(&format!("res_{}", i))
                .name(format!("Resource {}", i))
                .capacity(capacity)
        })
        .collect();

    // Tasks with multiple resource assignments
    for i in 0..task_count {
        let primary_res = i % resource_count;
        let secondary_res = (i + 1) % resource_count;

        let mut task = Task::new(&format!("task_{:04}", i))
            .name(format!("Task {}", i))
            .duration(utf8proj_core::Duration::days(2))
            .assign(&format!("res_{}", primary_res));

        // Some tasks need two resources
        if i % 3 == 0 {
            task = task.assign(&format!("res_{}", secondary_res));
        }

        // Add some dependencies to create structure
        if i > 0 && i % 5 == 0 {
            task = task.depends_on(&format!("task_{:04}", i - 1));
        }

        project.tasks.push(task);
    }

    project
}

/// Run a BDD benchmark comparing BDD analysis vs heuristic leveling
pub fn run_bdd_benchmark(
    scenario: BddScenario,
    task_count: usize,
    resource_count: usize,
) -> BddBenchmarkResult {
    // Generate project
    let project = generate_conflict_project(scenario, task_count, resource_count);

    // Schedule without leveling first
    let solver = CpmSolver::new();
    let schedule_start = Instant::now();
    let schedule = match solver.schedule(&project) {
        Ok(s) => s,
        Err(e) => {
            return BddBenchmarkResult {
                scenario: scenario.to_string(),
                task_count,
                resource_count,
                conflict_count: 0,
                schedule_time: schedule_start.elapsed(),
                bdd_analysis_time: Duration::ZERO,
                heuristic_level_time: Duration::ZERO,
                bdd_conflicts_found: 0,
                bdd_resolution_found: false,
                heuristic_conflicts_remaining: 0,
                bdd_nodes: 0,
                status: BddBenchmarkStatus::Error(e.to_string()),
            };
        }
    };
    let schedule_time = schedule_start.elapsed();

    // BDD analysis
    let bdd_start = Instant::now();
    let analyzer = BddConflictAnalyzer::new();
    let analysis = analyzer.analyze(&project, &schedule);
    let bdd_resolution = analyzer.find_optimal_resolution(&project, &schedule);
    let bdd_analysis_time = bdd_start.elapsed();

    // Heuristic leveling
    let calendar = project.calendars.first().cloned().unwrap_or_default();
    let heuristic_start = Instant::now();
    let leveling_result = level_resources(&project, &schedule, &calendar);
    let heuristic_level_time = heuristic_start.elapsed();

    BddBenchmarkResult {
        scenario: scenario.to_string(),
        task_count,
        resource_count,
        conflict_count: analysis.conflicts.len(),
        schedule_time,
        bdd_analysis_time,
        heuristic_level_time,
        bdd_conflicts_found: analysis.conflicts.len(),
        bdd_resolution_found: bdd_resolution.is_some(),
        heuristic_conflicts_remaining: leveling_result.unresolved_conflicts.len(),
        bdd_nodes: analysis.stats.nodes,
        status: BddBenchmarkStatus::Success,
    }
}

/// Run a series of BDD benchmarks
pub fn run_bdd_benchmark_series(
    scenario: BddScenario,
    sizes: &[(usize, usize)], // (task_count, resource_count)
) -> Vec<BddBenchmarkResult> {
    sizes
        .iter()
        .map(|&(tasks, resources)| {
            println!(
                "  Running {} with {} tasks, {} resources...",
                scenario, tasks, resources
            );
            run_bdd_benchmark(scenario, tasks, resources)
        })
        .collect()
}

/// Print BDD benchmark report
pub fn print_bdd_report(results: &[BddBenchmarkResult]) {
    println!();
    println!("╔════════════════════════════════════════════════════════════════════════════════════════════════════════════════╗");
    println!("║                                    BDD vs Heuristic Leveling Benchmark                                         ║");
    println!("╠════════════════════════════════════════════════════════════════════════════════════════════════════════════════╣");
    println!(
        "║ {:^10} │ {:^6} │ {:^5} │ {:^8} │ {:^10} │ {:^10} │ {:^8} │ {:^8} │ {:^8} │ {:^6} ║",
        "Scenario",
        "Tasks",
        "Res",
        "Conflicts",
        "BDD Time",
        "Heur Time",
        "BDD Res",
        "Heur Rem",
        "BDD Nodes",
        "Status"
    );
    println!("╠════════════════════════════════════════════════════════════════════════════════════════════════════════════════╣");

    for result in results {
        let bdd_ms = format!("{:.2}ms", result.bdd_analysis_time.as_secs_f64() * 1000.0);
        let heur_ms = format!(
            "{:.2}ms",
            result.heuristic_level_time.as_secs_f64() * 1000.0
        );
        let bdd_res = if result.bdd_resolution_found {
            "Yes"
        } else {
            "No"
        };

        println!(
            "║ {:^10} │ {:>6} │ {:>5} │ {:>8} │ {:>10} │ {:>10} │ {:^8} │ {:>8} │ {:>8} │ {:^6} ║",
            result.scenario,
            result.task_count,
            result.resource_count,
            result.bdd_conflicts_found,
            bdd_ms,
            heur_ms,
            bdd_res,
            result.heuristic_conflicts_remaining,
            result.bdd_nodes,
            result.status
        );
    }

    println!("╚════════════════════════════════════════════════════════════════════════════════════════════════════════════════╝");
    println!();

    // Summary
    let successful: Vec<_> = results
        .iter()
        .filter(|r| matches!(r.status, BddBenchmarkStatus::Success))
        .collect();

    if !successful.is_empty() {
        let total_conflicts: usize = successful.iter().map(|r| r.bdd_conflicts_found).sum();
        let bdd_resolved: usize = successful.iter().filter(|r| r.bdd_resolution_found).count();
        let avg_bdd_time: f64 = successful
            .iter()
            .map(|r| r.bdd_analysis_time.as_secs_f64())
            .sum::<f64>()
            / successful.len() as f64;
        let avg_heur_time: f64 = successful
            .iter()
            .map(|r| r.heuristic_level_time.as_secs_f64())
            .sum::<f64>()
            / successful.len() as f64;

        println!("Summary:");
        println!("  Successful runs: {}/{}", successful.len(), results.len());
        println!("  Total conflicts detected: {}", total_conflicts);
        println!(
            "  BDD found resolution: {}/{}",
            bdd_resolved,
            successful.len()
        );
        println!("  Avg BDD analysis time: {:.2}ms", avg_bdd_time * 1000.0);
        println!("  Avg heuristic time: {:.2}ms", avg_heur_time * 1000.0);

        if avg_heur_time > 0.0 {
            let speedup = avg_heur_time / avg_bdd_time;
            if speedup > 1.0 {
                println!("  BDD is {:.1}x faster than heuristic", speedup);
            } else {
                println!("  Heuristic is {:.1}x faster than BDD", 1.0 / speedup);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_resource_generates_conflicts() {
        let project = generate_single_resource_conflict(10);
        assert_eq!(project.tasks.len(), 10);
        assert_eq!(project.resources.len(), 1);

        // All tasks should have resource assignment
        for task in &project.tasks {
            assert!(!task.assigned.is_empty());
        }
    }

    #[test]
    fn multi_resource_distributes_tasks() {
        let project = generate_multi_resource_conflict(12, 3);
        assert_eq!(project.tasks.len(), 12);
        assert_eq!(project.resources.len(), 3);
    }

    #[test]
    fn resource_web_creates_dependencies() {
        let project = generate_resource_web(20, 4);
        assert_eq!(project.tasks.len(), 20);
        assert_eq!(project.resources.len(), 4);

        // Some tasks should have dependencies
        let with_deps = project
            .tasks
            .iter()
            .filter(|t| !t.depends.is_empty())
            .count();
        assert!(with_deps > 0);
    }

    #[test]
    fn benchmark_runs_successfully() {
        let result = run_bdd_benchmark(BddScenario::SingleResource, 5, 1);
        assert!(matches!(result.status, BddBenchmarkStatus::Success));
        assert!(result.bdd_conflicts_found > 0);
    }
}
