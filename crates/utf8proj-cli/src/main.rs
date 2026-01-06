//! utf8proj CLI - Project Scheduling Engine
//!
//! Command-line interface for parsing, scheduling, and rendering projects.

mod bench;
mod diagnostics;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::fs;
use std::io::Write;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

use utf8proj_core::{CollectingEmitter, Diagnostic, DiagnosticCode, DiagnosticEmitter, Scheduler};
use utf8proj_parser::parse_file;
use utf8proj_solver::{AnalysisConfig, CpmSolver, analyze_project, calculate_utilization};

use crate::diagnostics::{DiagnosticConfig, JsonEmitter, TerminalEmitter};

#[derive(Parser)]
#[command(name = "utf8proj")]
#[command(author, version, about = "Project scheduling engine", long_about = None)]
struct Cli {
    /// Verbose output
    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Parse and validate a project file (no scheduling output)
    Check {
        /// Input file path
        #[arg(value_name = "FILE")]
        file: std::path::PathBuf,

        /// Output format (text, json)
        #[arg(short, long, default_value = "text")]
        format: String,

        /// Strict mode: warnings become errors, hints become warnings
        #[arg(long)]
        strict: bool,

        /// Quiet mode: suppress all output except errors
        #[arg(short, long)]
        quiet: bool,
    },

    /// Schedule a project
    Schedule {
        /// Input file path
        #[arg(value_name = "FILE")]
        file: std::path::PathBuf,

        /// Output format (text, json)
        #[arg(short, long, default_value = "text")]
        format: String,

        /// Output file (stdout if not specified)
        #[arg(short, long)]
        output: Option<std::path::PathBuf>,

        /// Enable resource leveling
        #[arg(short, long)]
        leveling: bool,

        /// Show progress tracking information
        #[arg(short = 'p', long)]
        show_progress: bool,

        /// Strict mode: warnings become errors, hints become warnings
        #[arg(long)]
        strict: bool,

        /// Quiet mode: suppress all output except errors
        #[arg(short, long)]
        quiet: bool,
    },

    /// Generate a Gantt chart
    Gantt {
        /// Input file path
        #[arg(value_name = "FILE")]
        file: std::path::PathBuf,

        /// Output file path
        #[arg(short, long)]
        output: std::path::PathBuf,
    },

    /// Run performance benchmarks
    Benchmark {
        /// Benchmark type: synthetic topology or PSPLIB file
        #[arg(short, long, value_enum, default_value = "chain")]
        topology: bench::Topology,

        /// Number of tasks for synthetic benchmarks
        #[arg(short, long, default_value = "1000")]
        count: usize,

        /// Run a series of increasing sizes
        #[arg(short, long)]
        series: bool,

        /// Enable resource leveling during scheduling
        #[arg(short, long)]
        leveling: bool,

        /// PSPLIB file for validation benchmarks (future)
        #[arg(short, long)]
        file: Option<std::path::PathBuf>,
    },

    /// Run BDD conflict resolution benchmarks
    BddBenchmark {
        /// Scenario type for BDD benchmarks
        #[arg(short, long, value_enum, default_value = "single-resource")]
        scenario: bench::bdd::BddScenario,

        /// Number of tasks
        #[arg(short, long, default_value = "50")]
        tasks: usize,

        /// Number of resources
        #[arg(short, long, default_value = "5")]
        resources: usize,

        /// Run a series of increasing sizes
        #[arg(long)]
        series: bool,
    },
}

fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Check { file, format, strict, quiet }) => {
            cmd_check(&file, &format, strict, quiet)
        }
        Some(Commands::Schedule { file, format, output, leveling, show_progress, strict, quiet }) => {
            cmd_schedule(&file, &format, output.as_deref(), leveling, show_progress, strict, quiet)
        }
        Some(Commands::Gantt { file, output }) => cmd_gantt(&file, &output),
        Some(Commands::Benchmark {
            topology,
            count,
            series,
            leveling,
            file: _,
        }) => cmd_benchmark(topology, count, series, leveling),
        Some(Commands::BddBenchmark {
            scenario,
            tasks,
            resources,
            series,
        }) => cmd_bdd_benchmark(scenario, tasks, resources, series),
        None => {
            println!("utf8proj - Project Scheduling Engine");
            println!();
            println!("Usage: utf8proj <COMMAND>");
            println!();
            println!("Commands:");
            println!("  check      Parse and validate a project file");
            println!("  schedule   Schedule a project and output results");
            println!("  gantt      Generate a Gantt chart (SVG)");
            println!("  benchmark  Run performance benchmarks");
            println!();
            println!("Run 'utf8proj --help' for more information");
            Ok(())
        }
    }
}

/// Check command: parse and validate a project file
///
/// This is the fast validation entry point - it parses the file, schedules
/// (to enable cost analysis), and runs semantic analysis, but produces no
/// schedule output. Designed for CI pipelines, pre-commit hooks, and editors.
fn cmd_check(file: &std::path::Path, format: &str, strict: bool, quiet: bool) -> Result<()> {
    // Parse the file
    let project = parse_file(file)
        .with_context(|| format!("Failed to parse '{}'", file.display()))?;

    // Schedule the project (needed for cost analysis in diagnostics)
    let solver = CpmSolver::new();
    let schedule_result = solver.schedule(&project);

    // Run diagnostic analysis
    let analysis_config = AnalysisConfig::new().with_file(file);
    let mut collector = CollectingEmitter::new();

    // If scheduling failed due to infeasible constraints, emit E003
    if let Err(ref e) = schedule_result {
        use utf8proj_core::ScheduleError;
        if let ScheduleError::Infeasible(msg) = e {
            collector.emit(
                Diagnostic::new(
                    DiagnosticCode::E003InfeasibleConstraint,
                    format!("constraint cannot be satisfied"),
                )
                .with_file(file.to_path_buf())
                .with_note(msg.clone())
                .with_hint("check that constraints don't conflict with dependencies"),
            );
        }
    }

    let schedule = schedule_result.ok();
    analyze_project(&project, schedule.as_ref(), &analysis_config, &mut collector);

    // Configure diagnostic output
    let diag_config = DiagnosticConfig {
        strict,
        quiet,
        base_path: file.parent().map(|p| p.to_path_buf()),
    };

    // Emit diagnostics based on format
    let has_errors = match format {
        "json" => {
            let mut json_emitter = JsonEmitter::new(diag_config);
            for diag in collector.sorted() {
                json_emitter.emit(diag.clone());
            }

            // Output JSON diagnostics array
            let output = serde_json::json!({
                "file": file.display().to_string(),
                "diagnostics": json_emitter.to_json_value(),
            });
            println!("{}", serde_json::to_string_pretty(&output).unwrap());

            json_emitter.has_errors()
        }
        "text" | _ => {
            let mut term_emitter = TerminalEmitter::new(std::io::stderr(), diag_config);
            for diag in collector.sorted() {
                term_emitter.emit(diag.clone());
            }

            // Show summary if not quiet and no errors
            if !quiet && !term_emitter.has_errors() {
                eprintln!(
                    "Checked '{}': {} tasks, {} resources, {} profiles",
                    file.display(),
                    count_tasks(&project.tasks),
                    project.resources.len(),
                    project.profiles.len()
                );
            }

            term_emitter.has_errors()
        }
    };

    // Exit with error if there were errors
    if has_errors {
        return Err(anyhow::anyhow!("aborting due to previous error(s)"));
    }

    Ok(())
}

/// Schedule command: parse, schedule, and output results
fn cmd_schedule(
    file: &std::path::Path,
    format: &str,
    output: Option<&std::path::Path>,
    leveling: bool,
    show_progress: bool,
    strict: bool,
    quiet: bool,
) -> Result<()> {
    // Parse the file
    let project = parse_file(file)
        .with_context(|| format!("Failed to parse '{}'", file.display()))?;

    // Check feasibility first
    let solver = if leveling {
        CpmSolver::with_leveling()
    } else {
        CpmSolver::new()
    };
    let feasibility = solver.is_feasible(&project);

    if !feasibility.feasible {
        // Emit proper diagnostics for feasibility failures
        let diag_config = DiagnosticConfig {
            strict,
            quiet,
            base_path: file.parent().map(|p| p.to_path_buf()),
        };
        let mut term_emitter = TerminalEmitter::new(std::io::stderr(), diag_config.clone());

        for conflict in &feasibility.conflicts {
            use utf8proj_core::ConflictType;
            let diag = match conflict.conflict_type {
                ConflictType::ImpossibleConstraint => {
                    // Extract task info from the description for better error reporting
                    let note = conflict.description.clone();
                    Diagnostic::new(
                        DiagnosticCode::E003InfeasibleConstraint,
                        "constraint cannot be satisfied".to_string(),
                    )
                    .with_file(file.to_path_buf())
                    .with_note(note)
                    .with_hint("check that constraints don't conflict with dependencies")
                }
                ConflictType::CircularDependency => {
                    Diagnostic::new(
                        DiagnosticCode::E001CircularSpecialization, // Reuse for now
                        "circular dependency detected".to_string(),
                    )
                    .with_file(file.to_path_buf())
                    .with_note(conflict.description.clone())
                }
                _ => {
                    Diagnostic::new(
                        DiagnosticCode::E003InfeasibleConstraint,
                        format!("{:?}", conflict.conflict_type),
                    )
                    .with_file(file.to_path_buf())
                    .with_note(conflict.description.clone())
                }
            };
            term_emitter.emit(diag);
        }

        return Err(anyhow::anyhow!("Failed to generate schedule"));
    }

    // Schedule the project
    let schedule_result = solver.schedule(&project);

    // Run diagnostic analysis
    let analysis_config = AnalysisConfig::new()
        .with_file(file);

    // Collect diagnostics first, then emit in correct order
    let mut collector = CollectingEmitter::new();

    // If scheduling failed due to infeasible constraints, emit E003
    if let Err(ref e) = schedule_result {
        use utf8proj_core::ScheduleError;
        if let ScheduleError::Infeasible(msg) = e {
            collector.emit(
                Diagnostic::new(
                    DiagnosticCode::E003InfeasibleConstraint,
                    format!("constraint cannot be satisfied"),
                )
                .with_file(file.to_path_buf())
                .with_note(msg.clone())
                .with_hint("check that constraints don't conflict with dependencies"),
            );
        }
    }

    let schedule = match schedule_result {
        Ok(s) => s,
        Err(_) => {
            // Emit the collected E003 diagnostic before returning
            let diag_config = DiagnosticConfig {
                strict,
                quiet,
                base_path: file.parent().map(|p| p.to_path_buf()),
            };
            let mut term_emitter = TerminalEmitter::new(std::io::stderr(), diag_config);
            for diag in collector.sorted() {
                term_emitter.emit(diag.clone());
            }
            return Err(anyhow::anyhow!("Failed to generate schedule"));
        }
    };

    analyze_project(&project, Some(&schedule), &analysis_config, &mut collector);

    // Calculate and emit resource utilization (I003)
    if !project.resources.is_empty() {
        let calendar = project.calendars.first().cloned().unwrap_or_default();
        let utilization = calculate_utilization(&project, &schedule, &calendar);

        // Build utilization message
        let mut util_lines = Vec::new();
        for res_util in &utilization.resources {
            let status = if res_util.utilization_percent > 100.0 {
                "OVER"
            } else if res_util.utilization_percent > 80.0 {
                "HIGH"
            } else if res_util.utilization_percent < 20.0 && res_util.assigned_days > 0 {
                "LOW"
            } else if res_util.assigned_days == 0 {
                "IDLE"
            } else {
                ""
            };
            let status_suffix = if status.is_empty() {
                String::new()
            } else {
                format!(" [{}]", status)
            };
            util_lines.push(format!(
                "  {}: {:.0}% ({:.1}/{} days){}",
                res_util.resource_id,
                res_util.utilization_percent,
                res_util.used_days,
                res_util.total_days,
                status_suffix
            ));
        }

        let util_message = format!(
            "Resource utilization ({} - {})\n{}",
            utilization.schedule_start.format("%Y-%m-%d"),
            utilization.schedule_end.format("%Y-%m-%d"),
            util_lines.join("\n")
        );

        collector.emit(
            Diagnostic::new(DiagnosticCode::I003ResourceUtilization, util_message)
                .with_file(file.to_path_buf()),
        );
    }

    // Configure diagnostic output
    let diag_config = DiagnosticConfig {
        strict,
        quiet,
        base_path: file.parent().map(|p| p.to_path_buf()),
    };

    // Emit diagnostics based on format
    let has_errors = match format {
        "json" => {
            // For JSON, collect diagnostics and include in output
            let mut json_emitter = JsonEmitter::new(diag_config);
            for diag in collector.sorted() {
                json_emitter.emit(diag.clone());
            }

            // Format output with diagnostics
            let result = format_json_with_diagnostics(&project, &schedule, show_progress, &json_emitter)?;

            // Write output
            match output {
                Some(path) => {
                    let mut out_file = fs::File::create(path)
                        .with_context(|| format!("Failed to create output file '{}'", path.display()))?;
                    out_file.write_all(result.as_bytes())
                        .with_context(|| "Failed to write output")?;
                    if !quiet {
                        eprintln!("Schedule written to: {}", path.display());
                    }
                }
                None => {
                    println!("{}", result);
                }
            }

            json_emitter.has_errors()
        }
        "text" | _ => {
            // For text, emit diagnostics to stderr
            let mut term_emitter = TerminalEmitter::new(std::io::stderr(), diag_config);
            for diag in collector.sorted() {
                term_emitter.emit(diag.clone());
            }

            // Format schedule output
            if !quiet {
                let result = format_text(&project, &schedule, show_progress);

                // Write output
                match output {
                    Some(path) => {
                        let mut out_file = fs::File::create(path)
                            .with_context(|| format!("Failed to create output file '{}'", path.display()))?;
                        out_file.write_all(result.as_bytes())
                            .with_context(|| "Failed to write output")?;
                        eprintln!("Schedule written to: {}", path.display());
                    }
                    None => {
                        println!("{}", result);
                    }
                }
            }

            term_emitter.has_errors()
        }
    };

    // Exit with error if there were errors
    if has_errors {
        return Err(anyhow::anyhow!("aborting due to previous error(s)"));
    }

    Ok(())
}

/// Gantt command: generate SVG Gantt chart
fn cmd_gantt(file: &std::path::Path, output: &std::path::Path) -> Result<()> {
    // Parse the file
    let project = parse_file(file)
        .with_context(|| format!("Failed to parse '{}'", file.display()))?;

    // Schedule the project
    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project)
        .with_context(|| "Failed to generate schedule")?;

    // Use the renderer
    use utf8proj_core::Renderer;
    let renderer = utf8proj_render::SvgRenderer::new();
    let svg = renderer.render(&project, &schedule)
        .with_context(|| "Failed to render Gantt chart")?;

    // Write SVG to file
    fs::write(output, &svg)
        .with_context(|| format!("Failed to write SVG to '{}'", output.display()))?;

    println!("Gantt chart written to: {}", output.display());
    Ok(())
}

/// Count tasks recursively (including nested tasks)
fn count_tasks(tasks: &[utf8proj_core::Task]) -> usize {
    tasks.iter().map(|t| 1 + count_tasks(&t.children)).sum()
}

/// Format schedule as text table
fn format_text(project: &utf8proj_core::Project, schedule: &utf8proj_core::Schedule, show_progress: bool) -> String {
    let mut output = String::new();

    // Header
    output.push_str(&format!("Project: {}\n", project.name));
    output.push_str(&format!("Start: {}\n", project.start));
    output.push_str(&format!("End: {}\n", schedule.project_end));
    output.push_str(&format!("Duration: {} days\n", schedule.project_duration.as_days()));

    // Project status (I004)
    let status_icon = if schedule.project_variance_days > 5 {
        "🔴"
    } else if schedule.project_variance_days > 0 {
        "🟡"
    } else {
        "🟢"
    };
    let variance_str = if schedule.project_variance_days == 0 {
        "on schedule".to_string()
    } else if schedule.project_variance_days > 0 {
        format!("+{}d behind", schedule.project_variance_days)
    } else {
        format!("{}d ahead", schedule.project_variance_days.abs())
    };
    output.push_str(&format!(
        "Status: {}% complete, {} {}\n",
        schedule.project_progress, variance_str, status_icon
    ));

    // Earned Value (I005)
    let spi_icon = if schedule.spi >= 1.0 {
        "🟢"
    } else if schedule.spi >= 0.95 {
        "🟡"
    } else {
        "🔴"
    };
    let spi_status = if schedule.spi >= 1.0 {
        "on schedule"
    } else if schedule.spi >= 0.95 {
        "slightly behind"
    } else {
        "behind schedule"
    };
    output.push_str(&format!(
        "Earned Value: SPI {:.2} ({}) {} | EV {}% / PV {}%\n",
        schedule.spi, spi_status, spi_icon,
        schedule.earned_value, schedule.planned_value
    ));
    output.push('\n');

    // Critical path
    if !schedule.critical_path.is_empty() {
        output.push_str("Critical Path: ");
        output.push_str(&schedule.critical_path.join(" -> "));
        output.push_str("\n\n");
    }

    if show_progress {
        // Progress-aware output format with variance
        output.push_str(&format!(
            "{:<16} {:>6} {:<14} {:<12} {:<12} {:>8} {:>8} {}\n",
            "Task", "%Done", "Status", "Start", "Finish", "Remain", "Variance", "Critical"
        ));
        output.push_str(&format!("{}\n", "-".repeat(96)));

        // Sort tasks by start date
        let mut tasks: Vec<_> = schedule.tasks.values().collect();
        tasks.sort_by_key(|t| t.start);

        // Task rows with progress and variance
        for task in tasks {
            let critical = if task.is_critical { "*" } else { "" };
            let variance_str = if task.finish_variance_days == 0 {
                "—".to_string()
            } else if task.finish_variance_days > 0 {
                format!("+{}d", task.finish_variance_days)
            } else {
                format!("{}d", task.finish_variance_days)
            };
            let display_name = get_task_display_name(&project.tasks, &task.task_id);
            output.push_str(&format!(
                "{:<16} {:>5}% {:<14} {:<12} {:<12} {:>6}d {:>8} {}\n",
                truncate(&display_name, 16),
                task.percent_complete,
                format!("{}", task.status),
                task.forecast_start.format("%Y-%m-%d"),
                task.forecast_finish.format("%Y-%m-%d"),
                task.remaining_duration.as_days() as i64,
                variance_str,
                critical
            ));
        }
    } else {
        // Standard output format
        output.push_str(&format!(
            "{:<20} {:<12} {:<12} {:>8} {:>8} {}\n",
            "Task", "Start", "Finish", "Duration", "Slack", "Critical"
        ));
        output.push_str(&format!("{}\n", "-".repeat(76)));

        // Sort tasks by start date
        let mut tasks: Vec<_> = schedule.tasks.values().collect();
        tasks.sort_by_key(|t| t.start);

        // Task rows
        for task in tasks {
            let critical = if task.is_critical { "*" } else { "" };
            let display_name = get_task_display_name(&project.tasks, &task.task_id);
            output.push_str(&format!(
                "{:<20} {:<12} {:<12} {:>6}d {:>6}d {}\n",
                truncate(&display_name, 20),
                task.start.format("%Y-%m-%d"),
                task.finish.format("%Y-%m-%d"),
                task.duration.as_days() as i64,
                task.slack.as_days() as i64,
                critical
            ));
        }
    }

    output
}


/// Format schedule as JSON with diagnostics included
fn format_json_with_diagnostics(
    project: &utf8proj_core::Project,
    schedule: &utf8proj_core::Schedule,
    show_progress: bool,
    json_emitter: &JsonEmitter,
) -> Result<String> {
    // Create a summary structure for JSON output
    let summary = serde_json::json!({
        "diagnostics": json_emitter.to_json_value(),
        "schedule": {
            "project_name": project.name,
            "start": project.start.to_string(),
            "end": schedule.project_end.to_string(),
            "duration_days": schedule.project_duration.as_days(),
            "project_status": {
                "progress_percent": schedule.project_progress,
                "baseline_finish": schedule.project_baseline_finish.to_string(),
                "forecast_finish": schedule.project_forecast_finish.to_string(),
                "variance_days": schedule.project_variance_days,
            },
            "earned_value": {
                "earned_value": schedule.earned_value,
                "planned_value": schedule.planned_value,
                "spi": schedule.spi,
            },
            "tasks": schedule.tasks.values().map(|t| {
                // Get task info: (name/description, summary)
                let (description, summary) = find_task_info(&project.tasks, &t.task_id)
                    .unwrap_or_else(|| (t.task_id.clone(), None));
                let mut task_json = serde_json::json!({
                    "id": t.task_id,
                    "name": description,
                    "summary": summary,
                    "start": t.start.to_string(),
                    "finish": t.finish.to_string(),
                    "duration_days": t.duration.as_days(),
                    "is_critical": t.is_critical,
                });

                // Add progress fields if requested
                if show_progress {
                    task_json["progress"] = serde_json::json!({
                        "percent_complete": t.percent_complete,
                        "status": format!("{}", t.status),
                        "remaining_days": t.remaining_duration.as_days(),
                    });
                    task_json["variance"] = serde_json::json!({
                        "baseline_start": t.baseline_start.to_string(),
                        "baseline_finish": t.baseline_finish.to_string(),
                        "forecast_start": t.forecast_start.to_string(),
                        "forecast_finish": t.forecast_finish.to_string(),
                        "start_variance_days": t.start_variance_days,
                        "finish_variance_days": t.finish_variance_days,
                    });
                }
                task_json
            }).collect::<Vec<_>>(),
        },
    });

    serde_json::to_string_pretty(&summary)
        .with_context(|| "Failed to serialize schedule to JSON")
}

/// Truncate a string to max length with ellipsis
fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}...", &s[..max - 3])
    }
}

/// Find a task by ID in a nested task tree, returning (name/description, summary)
fn find_task_info(tasks: &[utf8proj_core::Task], id: &str) -> Option<(String, Option<String>)> {
    for task in tasks {
        if task.id == id {
            return Some((task.name.clone(), task.summary.clone()));
        }
        // For nested tasks, the ID might be like "parent.child"
        let prefix = format!("{}.", task.id);
        if id.starts_with(&prefix) {
            if let Some(result) = find_task_info(&task.children, id) {
                return Some(result);
            }
        }
        // Also check children directly (for non-prefixed lookups)
        if let Some(result) = find_task_info(&task.children, id) {
            return Some(result);
        }
    }
    None
}

/// Get the display name for a task using fallback: summary → name → id
fn get_task_display_name(tasks: &[utf8proj_core::Task], id: &str) -> String {
    if let Some((name, summary)) = find_task_info(tasks, id) {
        // Fallback: summary → name (description) → id
        if let Some(s) = summary {
            if !s.is_empty() {
                return s;
            }
        }
        if !name.is_empty() && name != id {
            return name;
        }
    }
    id.to_string()
}

/// Benchmark command: run performance benchmarks
fn cmd_benchmark(
    topology: bench::Topology,
    count: usize,
    series: bool,
    leveling: bool,
) -> Result<()> {
    println!("utf8proj Performance Benchmark");
    println!("==============================");
    println!();
    println!("Configuration:");
    println!("  Topology: {}", topology);
    println!("  Resource Leveling: {}", if leveling { "enabled" } else { "disabled" });
    println!();

    let results = if series {
        // Run a series of increasing sizes
        let sizes = match topology {
            bench::Topology::Chain => vec![100, 500, 1000, 5000, 10000, 50000],
            bench::Topology::Diamond => vec![100, 500, 1000, 5000, 10000, 50000],
            bench::Topology::Web => vec![100, 500, 1000, 2500, 5000, 10000],
        };
        println!("Running benchmark series: {:?}", sizes);
        println!();
        bench::run_benchmark_series(topology, &sizes, leveling)
    } else {
        // Single run
        println!("Running single benchmark with {} tasks...", count);
        println!();
        vec![bench::run_synthetic_benchmark(topology, count, leveling)]
    };

    bench::print_report(&results);

    // Check for any failures
    let failures: Vec<_> = results
        .iter()
        .filter(|r| !matches!(r.status, bench::BenchmarkStatus::Success))
        .collect();

    if !failures.is_empty() {
        println!("WARNING: {} benchmark(s) failed:", failures.len());
        for f in failures {
            println!("  - {} ({} tasks): {}", f.topology, f.task_count, f.status);
        }
    }

    Ok(())
}

/// BDD benchmark command: compare BDD vs heuristic leveling
fn cmd_bdd_benchmark(
    scenario: bench::bdd::BddScenario,
    tasks: usize,
    resources: usize,
    series: bool,
) -> Result<()> {
    println!("utf8proj BDD Conflict Resolution Benchmark");
    println!("==========================================");
    println!();
    println!("Configuration:");
    println!("  Scenario: {}", scenario);
    println!();

    let results = if series {
        // Run a series of increasing sizes
        let sizes: Vec<(usize, usize)> = match scenario {
            bench::bdd::BddScenario::SingleResource => {
                vec![(10, 1), (25, 1), (50, 1), (100, 1), (200, 1)]
            }
            bench::bdd::BddScenario::MultiResource => {
                vec![(20, 3), (50, 5), (100, 10), (200, 15), (500, 20)]
            }
            bench::bdd::BddScenario::ResourceWeb => {
                vec![(20, 4), (50, 8), (100, 12), (200, 16), (400, 20)]
            }
        };
        println!("Running benchmark series: {:?}", sizes);
        println!();
        bench::bdd::run_bdd_benchmark_series(scenario, &sizes)
    } else {
        // Single run
        println!("Running single benchmark with {} tasks, {} resources...", tasks, resources);
        println!();
        vec![bench::bdd::run_bdd_benchmark(scenario, tasks, resources)]
    };

    bench::bdd::print_bdd_report(&results);

    // Check for any failures
    let failures: Vec<_> = results
        .iter()
        .filter(|r| !matches!(r.status, bench::bdd::BddBenchmarkStatus::Success))
        .collect();

    if !failures.is_empty() {
        println!("WARNING: {} benchmark(s) failed:", failures.len());
        for f in failures {
            println!("  - {} ({} tasks): {}", f.scenario, f.task_count, f.status);
        }
    }

    Ok(())
}
