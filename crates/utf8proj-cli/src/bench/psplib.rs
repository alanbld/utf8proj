//! PSPLIB benchmark validation module
//!
//! Validates utf8proj against PSPLIB (Project Scheduling Problem Library),
//! the standard benchmark for Resource-Constrained Project Scheduling Problems.
//!
//! Reference: Kolisch & Sprecher (1996): "PSPLIB - A project scheduling problem library"
//! URL: https://www.om-db.wi.tum.de/psplib/

use chrono::NaiveDate;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::time::{Duration, Instant};

use utf8proj_core::{Calendar, Dependency, DependencyType, Project, Resource, ResourceRef, Scheduler, Task};
use utf8proj_solver::CpmSolver;

/// Result of validating a PSPLIB instance
#[derive(Debug)]
pub struct PsplibResult {
    pub instance_name: String,
    pub jobs: usize,
    pub resources: usize,
    pub computed_makespan: Option<u32>,
    pub optimal_makespan: Option<u32>,
    pub gap_percent: Option<f64>,
    pub schedule_time: Duration,
    pub status: PsplibStatus,
}

/// Status of PSPLIB validation
#[derive(Debug, Clone)]
pub enum PsplibStatus {
    /// Makespan matches optimal exactly
    Optimal,
    /// Makespan within acceptable gap (e.g., 5%)
    Acceptable(f64),
    /// Makespan exceeds acceptable gap
    Suboptimal(f64),
    /// No optimal known, but scheduled successfully
    Feasible,
    /// Scheduling failed
    Error(String),
}

impl std::fmt::Display for PsplibStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PsplibStatus::Optimal => write!(f, "OPTIMAL"),
            PsplibStatus::Acceptable(gap) => write!(f, "OK ({:.1}%)", gap),
            PsplibStatus::Suboptimal(gap) => write!(f, "SUBOPT ({:.1}%)", gap),
            PsplibStatus::Feasible => write!(f, "FEASIBLE"),
            PsplibStatus::Error(e) => write!(f, "ERROR: {}", e),
        }
    }
}

/// Parsed PSPLIB instance
#[derive(Debug)]
pub struct PsplibInstance {
    pub name: String,
    pub jobs: usize,
    pub num_resources: usize,
    pub horizon: u32,
    /// job_id -> list of successor job_ids
    pub precedence: HashMap<u32, Vec<u32>>,
    /// job_id -> duration
    pub durations: HashMap<u32, u32>,
    /// job_id -> {resource_id -> demand}
    pub demands: HashMap<u32, HashMap<String, u32>>,
    /// resource_id -> capacity
    pub capacities: HashMap<String, u32>,
}

impl PsplibInstance {
    /// Parse a PSPLIB .sm file
    pub fn parse(content: &str, name: &str) -> Result<Self, String> {
        let mut jobs = 0;
        let mut num_resources = 4;
        let mut horizon = 100;
        let mut precedence: HashMap<u32, Vec<u32>> = HashMap::new();
        let mut durations: HashMap<u32, u32> = HashMap::new();
        let mut demands: HashMap<u32, HashMap<String, u32>> = HashMap::new();
        let mut capacities: HashMap<String, u32> = HashMap::new();

        // Parse header
        for line in content.lines() {
            if line.contains("jobs (incl. supersource/sink )") {
                if let Some(num) = line.split(':').nth(1) {
                    jobs = num.trim().parse().unwrap_or(0);
                }
            } else if line.trim().starts_with("- renewable") {
                if let Some(num) = line.split(':').nth(1) {
                    num_resources = num.trim().parse().unwrap_or(4);
                }
            } else if line.contains("horizon") && !line.contains("****************") {
                if let Some(num) = line.split(':').nth(1) {
                    horizon = num.trim().parse().unwrap_or(100);
                }
            }
        }

        // Find and parse PRECEDENCE RELATIONS section
        let prec_start = content.find("PRECEDENCE RELATIONS:");
        let prec_end = content.find("REQUESTS/DURATIONS:");

        if let (Some(start), Some(end)) = (prec_start, prec_end) {
            let section = &content[start..end];
            for line in section.lines().skip(2) {
                // Skip "PRECEDENCE RELATIONS:" and column header line
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 3 {
                    if let Ok(job_id) = parts[0].parse::<u32>() {
                        let num_successors: usize = parts[2].parse().unwrap_or(0);
                        let successors: Vec<u32> = parts
                            .iter()
                            .skip(3)
                            .take(num_successors)
                            .filter_map(|s| s.parse().ok())
                            .collect();
                        precedence.insert(job_id, successors);
                    }
                }
            }
        }

        // Find and parse REQUESTS/DURATIONS section
        let req_start = content.find("REQUESTS/DURATIONS:");
        let req_end = content.find("RESOURCEAVAILABILITIES:");

        if let (Some(start), Some(end)) = (req_start, req_end) {
            let section = &content[start..end];
            for line in section.lines().skip(3) {
                // Skip "REQUESTS/DURATIONS:", column header, and separator line
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 3 {
                    if let Ok(job_id) = parts[0].parse::<u32>() {
                        // mode is parts[1], duration is parts[2]
                        let duration: u32 = parts[2].parse().unwrap_or(0);
                        durations.insert(job_id, duration);

                        // Resource demands start at parts[3]
                        let mut job_demands = HashMap::new();
                        for (i, demand_str) in
                            parts.iter().skip(3).take(num_resources).enumerate()
                        {
                            if let Ok(demand) = demand_str.parse::<u32>() {
                                if demand > 0 {
                                    job_demands.insert(format!("R{}", i + 1), demand);
                                }
                            }
                        }
                        demands.insert(job_id, job_demands);
                    }
                }
            }
        }

        // Find and parse RESOURCEAVAILABILITIES section
        if let Some(cap_start) = content.find("RESOURCEAVAILABILITIES:") {
            let section = &content[cap_start..];
            let lines: Vec<&str> = section.lines().collect();
            // Line 1: "RESOURCEAVAILABILITIES:"
            // Line 2: "  R 1  R 2  R 3  R 4"
            // Line 3: "   12   13    4   12"
            if lines.len() >= 3 {
                let caps: Vec<u32> = lines[2]
                    .split_whitespace()
                    .filter_map(|s| s.parse().ok())
                    .collect();
                for (i, cap) in caps.iter().take(num_resources).enumerate() {
                    capacities.insert(format!("R{}", i + 1), *cap);
                }
            }
        }

        // Fill in default capacities if missing
        for i in 0..num_resources {
            let key = format!("R{}", i + 1);
            capacities.entry(key).or_insert(10);
        }

        Ok(PsplibInstance {
            name: name.to_string(),
            jobs,
            num_resources,
            horizon,
            precedence,
            durations,
            demands,
            capacities,
        })
    }

    /// Convert to utf8proj Project
    pub fn to_project(&self) -> Project {
        let start_date = NaiveDate::from_ymd_opt(2026, 1, 5).unwrap();

        // Create a continuous calendar (7 days/week for PSPLIB)
        // TimeRange uses minutes from midnight (0-1440)
        let calendar = Calendar {
            id: "continuous".into(),
            name: "Continuous".into(),
            working_hours: vec![utf8proj_core::TimeRange {
                start: 0,      // 00:00
                end: 24 * 60,  // 24:00 (1440 minutes)
            }],
            working_days: vec![0, 1, 2, 3, 4, 5, 6], // All days
            holidays: vec![],
            exceptions: vec![],
        };

        let mut project = Project {
            id: self.name.clone(),
            name: self.name.clone(),
            start: start_date,
            end: None,
            status_date: None,
            calendar: "continuous".into(),
            currency: "USD".to_string(),
            tasks: vec![],
            resources: vec![],
            calendars: vec![calendar],
            profiles: vec![],
            traits: vec![],
            scenarios: vec![],
            attributes: HashMap::new(),
            cost_policy: utf8proj_core::CostPolicy::Midpoint,
            leveling_mode: utf8proj_core::LevelingMode::None,
            optimal_threshold: None,
            optimal_timeout_ms: None,
        };

        // Create resources
        // For PSPLIB, we model each resource unit as a separate resource
        // or use capacity to represent availability
        for (res_id, _capacity) in &self.capacities {
            project.resources.push(Resource {
                id: res_id.to_lowercase().into(),
                name: res_id.clone(),
                rate: None, // No cost rate
                capacity: 1.0, // Full capacity - we'll use assignment units for demand
                calendar: None,
                efficiency: 1.0,
                attributes: HashMap::new(),
                specializes: None,
                availability: None,
            });
        }

        // Create tasks (skip supersource job 1 and supersink which is the last job)
        let real_jobs: Vec<u32> = self
            .durations
            .iter()
            .filter(|(_, &d)| d > 0)
            .map(|(&id, _)| id)
            .collect();

        for job_id in &real_jobs {
            let duration = self.durations.get(job_id).copied().unwrap_or(1);

            // Find predecessors (jobs that have this job as successor)
            let predecessors: Vec<u32> = self
                .precedence
                .iter()
                .filter_map(|(&pred_id, successors)| {
                    if successors.contains(job_id) && self.durations.get(&pred_id).copied().unwrap_or(0) > 0
                    {
                        Some(pred_id)
                    } else {
                        None
                    }
                })
                .collect();

            let mut task = Task {
                id: format!("j{}", job_id).into(),
                name: format!("Job {}", job_id),
                summary: None,
                effort: None,
                duration: Some(utf8proj_core::Duration::days(i64::from(duration))),
                depends: vec![],
                assigned: vec![],
                priority: 500,
                constraints: vec![],
                milestone: false,
                children: vec![],
                complete: None,
                actual_start: None,
                actual_finish: None,
                explicit_remaining: None,
                status: None,
                regime: None,
                attributes: HashMap::new(),
            };

            // Add dependencies
            for pred_id in predecessors {
                task.depends.push(Dependency {
                    predecessor: format!("j{}", pred_id).into(),
                    dep_type: DependencyType::FinishToStart,
                    lag: None,
                });
            }

            // Add resource assignments
            // For PSPLIB, demand/capacity ratio determines assignment units
            if let Some(job_demands) = self.demands.get(job_id) {
                for (res_id, demand) in job_demands {
                    let capacity = self.capacities.get(res_id).copied().unwrap_or(10);
                    // PSPLIB demand is in units; we normalize to fraction of capacity
                    let units = (*demand as f32) / (capacity as f32);
                    task.assigned.push(ResourceRef {
                        resource_id: res_id.to_lowercase().into(),
                        units,
                    });
                }
            }

            project.tasks.push(task);
        }

        project
    }
}

/// Parse optimal solutions file (e.g., j30opt.sm)
pub fn parse_optimal_solutions(content: &str) -> HashMap<String, u32> {
    let mut solutions = HashMap::new();

    for line in content.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        // Format: param instance makespan
        // e.g., "1 1 43" means j301_1 has optimal makespan 43
        if parts.len() >= 3 {
            if let (Ok(param), Ok(instance), Ok(makespan)) = (
                parts[0].parse::<u32>(),
                parts[1].parse::<u32>(),
                parts[2].parse::<u32>(),
            ) {
                // Instance naming convention: j30{param}_{instance}
                let name = format!("j30{}_{}", param, instance);
                solutions.insert(name, makespan);
            }
        }
    }

    solutions
}

/// Validate a single PSPLIB instance
pub fn validate_instance(
    instance: &PsplibInstance,
    optimal: Option<u32>,
    acceptable_gap: f64,
) -> PsplibResult {
    let start = Instant::now();

    let project = instance.to_project();
    let solver = CpmSolver::with_leveling();

    let (computed_makespan, status) = match solver.schedule(&project) {
        Ok(schedule) => {
            let makespan = schedule.project_duration.as_days() as u32;
            let status = match optimal {
                Some(opt) => {
                    let gap = ((makespan as f64 - opt as f64) / opt as f64) * 100.0;
                    if makespan == opt {
                        PsplibStatus::Optimal
                    } else if gap <= acceptable_gap {
                        PsplibStatus::Acceptable(gap)
                    } else {
                        PsplibStatus::Suboptimal(gap)
                    }
                }
                None => PsplibStatus::Feasible,
            };
            (Some(makespan), status)
        }
        Err(e) => (None, PsplibStatus::Error(e.to_string())),
    };

    let schedule_time = start.elapsed();
    let gap_percent = match (computed_makespan, optimal) {
        (Some(c), Some(o)) => Some(((c as f64 - o as f64) / o as f64) * 100.0),
        _ => None,
    };

    PsplibResult {
        instance_name: instance.name.clone(),
        jobs: instance.jobs,
        resources: instance.num_resources,
        computed_makespan,
        optimal_makespan: optimal,
        gap_percent,
        schedule_time,
        status,
    }
}

/// Validate all PSPLIB instances in a directory
pub fn validate_directory(
    dir_path: &Path,
    optimal_solutions: &HashMap<String, u32>,
    acceptable_gap: f64,
) -> Vec<PsplibResult> {
    let mut results = Vec::new();

    let entries: Vec<_> = fs::read_dir(dir_path)
        .map(|rd| rd.filter_map(|e| e.ok()).collect())
        .unwrap_or_default();

    for entry in entries {
        let path = entry.path();
        if path.extension().map(|e| e == "sm").unwrap_or(false) {
            let name = path.file_stem().unwrap_or_default().to_string_lossy();

            // Skip solution files
            if name.contains("opt") || name.contains("hrs") || name.contains("lb") {
                continue;
            }

            if let Ok(content) = fs::read_to_string(&path) {
                match PsplibInstance::parse(&content, &name) {
                    Ok(instance) => {
                        let optimal = optimal_solutions.get(&name.to_string()).copied();
                        let result = validate_instance(&instance, optimal, acceptable_gap);
                        results.push(result);
                    }
                    Err(e) => {
                        results.push(PsplibResult {
                            instance_name: name.to_string(),
                            jobs: 0,
                            resources: 0,
                            computed_makespan: None,
                            optimal_makespan: None,
                            gap_percent: None,
                            schedule_time: Duration::ZERO,
                            status: PsplibStatus::Error(format!("Parse error: {}", e)),
                        });
                    }
                }
            }
        }
    }

    // Sort by instance name for consistent output
    results.sort_by(|a, b| a.instance_name.cmp(&b.instance_name));
    results
}

/// Print PSPLIB validation report
pub fn print_psplib_report(results: &[PsplibResult]) {
    println!();
    println!("╔════════════════════════════════════════════════════════════════════════════════════╗");
    println!("║                         PSPLIB Validation Report                                   ║");
    println!("╠════════════════════════════════════════════════════════════════════════════════════╣");
    println!(
        "║ {:^15} │ {:^6} │ {:^8} │ {:^8} │ {:^8} │ {:^12} │ {:^12} ║",
        "Instance", "Jobs", "Computed", "Optimal", "Gap %", "Time", "Status"
    );
    println!("╠════════════════════════════════════════════════════════════════════════════════════╣");

    for result in results {
        let computed = result
            .computed_makespan
            .map(|m| m.to_string())
            .unwrap_or_else(|| "-".to_string());
        let optimal = result
            .optimal_makespan
            .map(|m| m.to_string())
            .unwrap_or_else(|| "-".to_string());
        let gap = result
            .gap_percent
            .map(|g| format!("{:.1}%", g))
            .unwrap_or_else(|| "-".to_string());
        let time = format!("{:.2}ms", result.schedule_time.as_secs_f64() * 1000.0);

        println!(
            "║ {:^15} │ {:>6} │ {:>8} │ {:>8} │ {:>8} │ {:>12} │ {:^12} ║",
            result.instance_name, result.jobs, computed, optimal, gap, time, result.status
        );
    }

    println!("╚════════════════════════════════════════════════════════════════════════════════════╝");
    println!();

    // Summary statistics
    let total = results.len();
    let optimal_count = results
        .iter()
        .filter(|r| matches!(r.status, PsplibStatus::Optimal))
        .count();
    let acceptable_count = results
        .iter()
        .filter(|r| matches!(r.status, PsplibStatus::Acceptable(_)))
        .count();
    let suboptimal_count = results
        .iter()
        .filter(|r| matches!(r.status, PsplibStatus::Suboptimal(_)))
        .count();
    let feasible_count = results
        .iter()
        .filter(|r| matches!(r.status, PsplibStatus::Feasible))
        .count();
    let error_count = results
        .iter()
        .filter(|r| matches!(r.status, PsplibStatus::Error(_)))
        .count();

    let gaps: Vec<f64> = results.iter().filter_map(|r| r.gap_percent).collect();
    let avg_gap = if gaps.is_empty() {
        0.0
    } else {
        gaps.iter().sum::<f64>() / gaps.len() as f64
    };

    let total_time: f64 = results.iter().map(|r| r.schedule_time.as_secs_f64()).sum();

    println!("Summary:");
    println!("  Total instances: {}", total);
    if total > 0 {
        println!(
            "  Optimal: {} ({:.1}%)",
            optimal_count,
            (optimal_count as f64 / total as f64) * 100.0
        );
        println!(
            "  Acceptable (≤5% gap): {} ({:.1}%)",
            acceptable_count,
            (acceptable_count as f64 / total as f64) * 100.0
        );
        println!("  Suboptimal: {}", suboptimal_count);
        println!("  Feasible (no optimal known): {}", feasible_count);
        println!("  Errors: {}", error_count);
    }
    println!("  Average gap: {:.2}%", avg_gap);
    println!("  Total time: {:.2}s", total_time);
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_PSPLIB: &str = r#"
************************************************************************
file with calculation calculation calculation
************************************************************************
projects                      :  1
jobs (incl. supersource/sink ):  32
horizon                       :  115
RESOURCES
  - renewable                 :  4   R
  - nonrenewable              :  0   N
  - doubly constrained        :  0   D
************************************************************************
PROJECT INFORMATION:
pronr.  #jobs rel.date duedate tardcost  MPM-Time
    1     30      0       41        5       41
************************************************************************
PRECEDENCE RELATIONS:
jobnr.    #modes  #successors   successors
   1        1          3           2   3   4
   2        1          3           7   8  10
   3        1          3           5   6  12
   4        1          2          11  14
   5        1          2          16  18
   6        1          2           9  11
   7        1          1          13
   8        1          2          13  23
   9        1          2          17  21
  10        1          3          15  16  22
  11        1          2          13  23
  12        1          3          14  15  22
  13        1          1          20
  14        1          3          16  17  24
  15        1          2          19  20
  16        1          1          26
  17        1          2          19  26
  18        1          3          21  22  23
  19        1          2          25  29
  20        1          3          21  25  26
  21        1          2          24  29
  22        1          2          24  28
  23        1          3          27  28  30
  24        1          2          25  27
  25        1          1          30
  26        1          2          27  28
  27        1          1          29
  28        1          1          31
  29        1          1          31
  30        1          1          31
  31        1          1          32
  32        1          0
************************************************************************
REQUESTS/DURATIONS:
jobnr. mode duration  R 1  R 2  R 3  R 4
------------------------------------------------------------------------
  1      1     0       0    0    0    0
  2      1     3       7    3    2    6
  3      1     9       5    4    1    1
  4      1     7       4    4    5    2
  5      1     9       3    1    3    5
  6      1     1       3    2    3    6
  7      1     6       5    2    7    1
  8      1     5       1    9    3    3
  9      1     3       4    4    7    2
 10      1     4       2    6    4    8
 11      1     5       1   10    9    5
 12      1     5       5    3    5    4
 13      1     7       6    3    2    5
 14      1     4       6    8    7    2
 15      1     9       6    3    8    2
 16      1     3       1    2    2    6
 17      1     4       3    5    5    5
 18      1     9       6    2    5    2
 19      1     9       9    6   10    2
 20      1     9       5    8    7    7
 21      1     4       4    1    2    8
 22      1     8       4    5    6    1
 23      1     4       1    8    9    4
 24      1     5       9    5    8    6
 25      1     6       2    1    7   10
 26      1     5       7    1    8    8
 27      1     4       2    3    1    8
 28      1     3       5    9    3    2
 29      1     5       2    2    6    3
 30      1     9       7    6    6    9
 31      1     3       3    3    8    3
 32      1     0       0    0    0    0
************************************************************************
RESOURCEAVAILABILITIES:
  R 1  R 2  R 3  R 4
   12   13   12   13
************************************************************************
"#;

    #[test]
    fn test_parse_psplib_instance() {
        let instance = PsplibInstance::parse(SAMPLE_PSPLIB, "test_instance").unwrap();

        assert_eq!(instance.name, "test_instance");
        assert_eq!(instance.jobs, 32);
        assert_eq!(instance.num_resources, 4);

        // Check job 2: duration 3, demands R1=7, R2=3, R3=2, R4=6
        assert_eq!(instance.durations.get(&2), Some(&3));
        // Note: Only demands > 0 are stored
        if let Some(j2_demands) = instance.demands.get(&2) {
            // R1=7, R2=3 should be present
            assert!(j2_demands.get("R1").is_some() || j2_demands.get("R2").is_some());
        }

        // Check capacities
        assert_eq!(instance.capacities.get("R1"), Some(&12));
        assert_eq!(instance.capacities.get("R2"), Some(&13));

        // Check precedence: job 1 has successors 2, 3, 4
        // The parser should have found precedence relationships
        assert!(!instance.precedence.is_empty(), "Precedence should not be empty");

        // Job 1 (supersource) should have successors
        if let Some(j1_succ) = instance.precedence.get(&1) {
            assert!(j1_succ.contains(&2));
            assert!(j1_succ.contains(&3));
            assert!(j1_succ.contains(&4));
        } else {
            // Debug: print what we found
            eprintln!("Precedence keys found: {:?}", instance.precedence.keys().collect::<Vec<_>>());
            panic!("Job 1 not found in precedence map");
        }
    }

    #[test]
    fn test_convert_to_project() {
        let instance = PsplibInstance::parse(SAMPLE_PSPLIB, "test_instance").unwrap();
        let project = instance.to_project();

        // Should have 30 real jobs (excluding supersource/sink with duration 0)
        assert_eq!(project.tasks.len(), 30);

        // Check that resources were created
        assert_eq!(project.resources.len(), 4);

        // Check first real task (j2)
        let j2 = project.tasks.iter().find(|t| t.id == "j2").unwrap();
        assert_eq!(j2.duration.as_ref().unwrap().as_days(), 3.0);
        assert!(!j2.assigned.is_empty());
    }

    #[test]
    fn test_validate_instance_schedules() {
        let instance = PsplibInstance::parse(SAMPLE_PSPLIB, "test_instance").unwrap();
        let result = validate_instance(&instance, Some(41), 5.0);

        assert!(result.computed_makespan.is_some());
        // The heuristic may not find optimal, but should produce a feasible schedule
        assert!(!matches!(result.status, PsplibStatus::Error(_)));
    }
}
