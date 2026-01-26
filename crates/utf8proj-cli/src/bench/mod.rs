//! Benchmarking module for utf8proj
//!
//! Provides synthetic stress tests, BDD comparison, and PSPLIB validation benchmarks.

#![allow(dead_code)] // Fields/variants for future PSPLIB validation

pub mod bdd;
pub mod psplib;
pub mod synthetic;

use std::time::{Duration, Instant};

/// Topology types for synthetic benchmarks
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Topology {
    /// Linear chain: A -> B -> C -> ... (tests recursion depth)
    Chain,
    /// Diamond: Start -> [N parallel] -> End (tests memory/allocation)
    Diamond,
    /// Random DAG with high connectivity (tests cycle detection)
    Web,
}

impl std::fmt::Display for Topology {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Topology::Chain => write!(f, "Chain"),
            Topology::Diamond => write!(f, "Diamond"),
            Topology::Web => write!(f, "Web"),
        }
    }
}

/// Result of a benchmark run
#[derive(Debug)]
pub struct BenchmarkResult {
    pub topology: String,
    pub task_count: usize,
    pub generation_time: Duration,
    pub schedule_time: Duration,
    pub total_time: Duration,
    pub status: BenchmarkStatus,
    pub project_duration_days: Option<f64>,
    pub critical_path_length: Option<usize>,
}

/// Status of a benchmark run
#[derive(Debug)]
pub enum BenchmarkStatus {
    Success,
    StackOverflow,
    Timeout,
    Error(String),
}

impl std::fmt::Display for BenchmarkStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BenchmarkStatus::Success => write!(f, "OK"),
            BenchmarkStatus::StackOverflow => write!(f, "STACK OVERFLOW"),
            BenchmarkStatus::Timeout => write!(f, "TIMEOUT"),
            BenchmarkStatus::Error(e) => write!(f, "ERROR: {}", e),
        }
    }
}

/// Run a synthetic benchmark
pub fn run_synthetic_benchmark(
    topology: Topology,
    task_count: usize,
    with_leveling: bool,
) -> BenchmarkResult {
    use utf8proj_core::Scheduler;
    use utf8proj_solver::CpmSolver;

    let total_start = Instant::now();

    // Generate project
    let gen_start = Instant::now();
    let project = match topology {
        Topology::Chain => synthetic::generate_chain(task_count),
        Topology::Diamond => synthetic::generate_diamond(task_count),
        Topology::Web => synthetic::generate_web(task_count),
    };
    let generation_time = gen_start.elapsed();

    // Schedule project
    let schedule_start = Instant::now();
    let solver = if with_leveling {
        CpmSolver::with_leveling()
    } else {
        CpmSolver::new()
    };

    let (status, project_duration_days, critical_path_length) = match solver.schedule(&project) {
        Ok(schedule) => (
            BenchmarkStatus::Success,
            Some(schedule.project_duration.as_days()),
            Some(schedule.critical_path.len()),
        ),
        Err(e) => {
            let err_str = e.to_string();
            if err_str.contains("stack overflow") {
                (BenchmarkStatus::StackOverflow, None, None)
            } else {
                (BenchmarkStatus::Error(err_str), None, None)
            }
        }
    };
    let schedule_time = schedule_start.elapsed();

    let total_time = total_start.elapsed();

    BenchmarkResult {
        topology: topology.to_string(),
        task_count,
        generation_time,
        schedule_time,
        total_time,
        status,
        project_duration_days,
        critical_path_length,
    }
}

/// Print a formatted benchmark report
pub fn print_report(results: &[BenchmarkResult]) {
    println!();
    println!("╔══════════════════════════════════════════════════════════════════════════════════════════════════╗");
    println!("║                               utf8proj Benchmark Report                                          ║");
    println!("╠══════════════════════════════════════════════════════════════════════════════════════════════════╣");
    println!(
        "║ {:^10} │ {:^8} │ {:^12} │ {:^12} │ {:^12} │ {:^10} │ {:^12} ║",
        "Topology", "Tasks", "Generate", "Schedule", "Total", "Crit.Path", "Status"
    );
    println!("╠══════════════════════════════════════════════════════════════════════════════════════════════════╣");

    for result in results {
        let gen_ms = format!("{:.2}ms", result.generation_time.as_secs_f64() * 1000.0);
        let sched_ms = format!("{:.2}ms", result.schedule_time.as_secs_f64() * 1000.0);
        let total_ms = format!("{:.2}ms", result.total_time.as_secs_f64() * 1000.0);
        let crit_path = result
            .critical_path_length
            .map(|c| c.to_string())
            .unwrap_or_else(|| "-".to_string());

        println!(
            "║ {:^10} │ {:>8} │ {:>12} │ {:>12} │ {:>12} │ {:>10} │ {:^12} ║",
            result.topology,
            result.task_count,
            gen_ms,
            sched_ms,
            total_ms,
            crit_path,
            result.status
        );
    }

    println!("╚══════════════════════════════════════════════════════════════════════════════════════════════════╝");
    println!();

    // Print summary statistics
    let successful: Vec<_> = results
        .iter()
        .filter(|r| matches!(r.status, BenchmarkStatus::Success))
        .collect();
    if !successful.is_empty() {
        let total_tasks: usize = successful.iter().map(|r| r.task_count).sum();
        let total_schedule_time: f64 = successful
            .iter()
            .map(|r| r.schedule_time.as_secs_f64())
            .sum();
        let avg_tasks_per_sec = total_tasks as f64 / total_schedule_time;

        println!("Summary:");
        println!("  Successful runs: {}/{}", successful.len(), results.len());
        println!("  Total tasks scheduled: {}", total_tasks);
        println!("  Average throughput: {:.0} tasks/sec", avg_tasks_per_sec);
    }
}

/// Run a series of benchmarks with increasing sizes
pub fn run_benchmark_series(
    topology: Topology,
    sizes: &[usize],
    with_leveling: bool,
) -> Vec<BenchmarkResult> {
    sizes
        .iter()
        .map(|&size| {
            println!("  Running {} with {} tasks...", topology, size);
            run_synthetic_benchmark(topology, size, with_leveling)
        })
        .collect()
}
