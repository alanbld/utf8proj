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
use utf8proj_solver::{
    analyze_project, calculate_utilization, level_resources_with_options, AnalysisConfig,
    CpmSolver, LevelingOptions,
};

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

        /// Show only calendar diagnostics (C001-C023)
        #[arg(long)]
        calendars: bool,
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

        /// Enable resource leveling (explicit opt-in, RFC-0003)
        #[arg(short, long)]
        leveling: bool,

        /// Maximum project delay factor when leveling (e.g., 1.5 = 50% max increase)
        #[arg(long)]
        max_delay_factor: Option<f64>,

        /// Show progress tracking information
        #[arg(short = 'p', long)]
        show_progress: bool,

        /// Strict mode: warnings become errors, hints become warnings
        #[arg(long)]
        strict: bool,

        /// Quiet mode: suppress all output except errors
        #[arg(short, long)]
        quiet: bool,

        /// Show task IDs instead of display names
        #[arg(long)]
        task_ids: bool,

        /// Verbose output: show both task ID and display name
        #[arg(short = 'V', long)]
        verbose: bool,

        /// Task name column width (default: 40)
        #[arg(short = 'w', long, default_value = "40")]
        width: usize,

        /// Show only calendar diagnostics (C001-C023)
        #[arg(long)]
        calendars: bool,

        /// Status date for progress-aware scheduling (RFC-0004)
        /// Format: YYYY-MM-DD. Overrides project.status_date.
        #[arg(long, value_name = "DATE")]
        as_of: Option<String>,
    },

    /// Generate a Gantt chart
    Gantt {
        /// Input file path
        #[arg(value_name = "FILE")]
        file: std::path::PathBuf,

        /// Output file path
        #[arg(short, long)]
        output: std::path::PathBuf,

        /// Output format (svg, mermaid, plantuml, xlsx, html)
        #[arg(short, long, default_value = "svg")]
        format: String,

        /// Show task IDs instead of display names
        #[arg(long)]
        task_ids: bool,

        /// Verbose output: show both task ID and display name
        #[arg(short = 'V', long)]
        verbose: bool,

        /// Task name column width (default: 40)
        #[arg(short = 'w', long, default_value = "40")]
        width: usize,

        /// Currency symbol for Excel export (default: EUR)
        #[arg(long, default_value = "EUR")]
        currency: String,

        /// Number of weeks to show in Excel schedule (default: 40)
        #[arg(long, default_value = "40")]
        weeks: u32,

        /// Include Calendar Analysis sheet in Excel export (shows weekend/holiday impact)
        #[arg(long)]
        include_calendar: bool,

        /// Include Diagnostics sheet in Excel export
        #[arg(long)]
        include_diagnostics: bool,

        /// Focus pattern(s) to expand specific task hierarchies (HTML format only).
        /// Matches task IDs by prefix, name by contains, or glob patterns.
        /// Multiple patterns can be comma-separated (e.g., "6.3.2,8.6").
        #[arg(long)]
        focus: Option<String>,

        /// Context depth for non-focused tasks (default: 1, 0 = hide all non-focused)
        #[arg(long, default_value = "1")]
        context_depth: usize,

        /// Use daily granularity for Excel export (one column per day, shows weekends/holidays)
        #[arg(long)]
        daily: bool,

        /// Number of days to show in daily Excel schedule (default: 60)
        #[arg(long, default_value = "60")]
        days: u32,
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

    /// Classify tasks by categories (RFC-0011)
    Classify {
        /// Input file path
        #[arg(value_name = "FILE")]
        file: std::path::PathBuf,

        /// Classification method
        #[arg(short, long, default_value = "status")]
        by: String,
    },

    /// Fix issues in project files
    Fix {
        #[command(subcommand)]
        fix_command: FixCommands,
    },

    /// Initialize a new project file with a working example
    Init {
        /// Project name (default: "my-project")
        #[arg(value_name = "NAME", default_value = "my-project")]
        name: String,

        /// Output directory (default: current directory)
        #[arg(short, long)]
        output: Option<std::path::PathBuf>,
    },
}

#[derive(Subcommand)]
enum FixCommands {
    /// Propagate container dependencies to children (fixes W014)
    ContainerDeps {
        /// Input file path
        #[arg(value_name = "FILE")]
        file: std::path::PathBuf,

        /// Output file path (default: stdout)
        #[arg(short, long)]
        output: Option<std::path::PathBuf>,

        /// Modify file in-place (requires explicit flag for safety)
        #[arg(long)]
        in_place: bool,
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
        Some(Commands::Check {
            file,
            format,
            strict,
            quiet,
            calendars,
        }) => cmd_check(&file, &format, strict, quiet, calendars),
        Some(Commands::Schedule {
            file,
            format,
            output,
            leveling,
            max_delay_factor,
            show_progress,
            strict,
            quiet,
            task_ids,
            verbose,
            width,
            calendars,
            as_of,
        }) => cmd_schedule(
            &file,
            &format,
            output.as_deref(),
            leveling,
            max_delay_factor,
            show_progress,
            strict,
            quiet,
            task_ids,
            verbose,
            width,
            calendars,
            as_of.as_deref(),
        ),
        Some(Commands::Gantt {
            file,
            output,
            format,
            task_ids,
            verbose,
            width,
            currency,
            weeks,
            include_calendar,
            include_diagnostics,
            focus,
            context_depth,
            daily,
            days,
        }) => cmd_gantt(
            &file,
            &output,
            &format,
            task_ids,
            verbose,
            width,
            &currency,
            weeks,
            include_calendar,
            include_diagnostics,
            focus.as_deref(),
            context_depth,
            daily,
            days,
        ),
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
        Some(Commands::Classify { file, by }) => cmd_classify(&file, &by),
        Some(Commands::Fix { fix_command }) => match fix_command {
            FixCommands::ContainerDeps {
                file,
                output,
                in_place,
            } => cmd_fix_container_deps(&file, output.as_deref(), in_place),
        },
        Some(Commands::Init { name, output }) => cmd_init(&name, output.as_deref()),
        None => {
            println!("utf8proj - Project Scheduling Engine");
            println!();
            println!("Usage: utf8proj <COMMAND>");
            println!();
            println!("Commands:");
            println!("  init       Initialize a new project file");
            println!("  check      Parse and validate a project file");
            println!("  schedule   Schedule a project and output results");
            println!("  gantt      Generate a Gantt chart (SVG)");
            println!("  classify   Classify tasks by categories (RFC-0011)");
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
fn cmd_check(
    file: &std::path::Path,
    format: &str,
    strict: bool,
    quiet: bool,
    calendars: bool,
) -> Result<()> {
    // Parse the file
    let project =
        parse_file(file).with_context(|| format!("Failed to parse '{}'", file.display()))?;

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
    analyze_project(
        &project,
        schedule.as_ref(),
        &analysis_config,
        &mut collector,
    );

    // Configure diagnostic output
    let diag_config = DiagnosticConfig {
        strict,
        quiet,
        base_path: file.parent().map(|p| p.to_path_buf()),
    };

    // Filter diagnostics if --calendars flag is set (clone to get owned Diagnostics)
    let diagnostics: Vec<Diagnostic> = if calendars {
        collector
            .sorted()
            .into_iter()
            .filter(|d| d.code.as_str().starts_with("C"))
            .cloned()
            .collect()
    } else {
        collector.sorted().into_iter().cloned().collect()
    };

    // Emit diagnostics based on format
    let has_errors = match format {
        "json" => {
            let mut json_emitter = JsonEmitter::new(diag_config);
            for diag in diagnostics {
                json_emitter.emit(diag);
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
            for diag in diagnostics {
                term_emitter.emit(diag);
            }

            // Show summary if not quiet and no errors
            if !quiet && !term_emitter.has_errors() {
                if calendars {
                    eprintln!(
                        "Calendar check '{}': {} calendars analyzed",
                        file.display(),
                        project.calendars.len()
                    );
                } else {
                    eprintln!(
                        "Checked '{}': {} tasks, {} resources, {} profiles",
                        file.display(),
                        count_tasks(&project.tasks),
                        project.resources.len(),
                        project.profiles.len()
                    );
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

/// Schedule command: parse, schedule, and output results
fn cmd_schedule(
    file: &std::path::Path,
    format: &str,
    output: Option<&std::path::Path>,
    leveling: bool,
    max_delay_factor: Option<f64>,
    show_progress: bool,
    strict: bool,
    quiet: bool,
    task_ids: bool,
    verbose: bool,
    width: usize,
    calendars: bool,
    as_of: Option<&str>,
) -> Result<()> {
    // Parse the file
    let project =
        parse_file(file).with_context(|| format!("Failed to parse '{}'", file.display()))?;

    // Parse --as-of date if provided (RFC-0004)
    let status_date_override = if let Some(date_str) = as_of {
        Some(
            chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d").with_context(|| {
                format!(
                    "Invalid date format for --as-of: '{}'. Expected YYYY-MM-DD.",
                    date_str
                )
            })?,
        )
    } else {
        None
    };

    // Create solver with status date override if provided (RFC-0004: C-01)
    let solver = if let Some(date) = status_date_override {
        CpmSolver::with_status_date(date)
    } else {
        CpmSolver::new()
    };

    // Check feasibility first
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
                _ => Diagnostic::new(
                    DiagnosticCode::E003InfeasibleConstraint,
                    format!("{:?}", conflict.conflict_type),
                )
                .with_file(file.to_path_buf())
                .with_note(conflict.description.clone()),
            };
            term_emitter.emit(diag);
        }

        return Err(anyhow::anyhow!("Failed to generate schedule"));
    }

    // Schedule the project
    let schedule_result = solver.schedule(&project);

    // Run diagnostic analysis
    let analysis_config = AnalysisConfig::new().with_file(file);

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

    let base_schedule = match schedule_result {
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

    // Apply resource leveling if enabled (RFC-0003: explicit opt-in)
    let (schedule, leveling_diagnostics) = if leveling {
        let calendar = project.calendars.first().cloned().unwrap_or_default();
        let options = LevelingOptions {
            strategy: utf8proj_solver::LevelingStrategy::CriticalPathFirst,
            max_project_delay_factor: max_delay_factor,
        };
        let result = level_resources_with_options(&project, &base_schedule, &calendar, &options);

        // Report leveling metrics if not quiet
        if !quiet {
            eprintln!(
                "Leveling: {} task(s) delayed, project duration {} by {} day(s)",
                result.metrics.tasks_delayed,
                if result.project_extended {
                    "increased"
                } else {
                    "unchanged"
                },
                result.metrics.project_duration_increase
            );
        }

        (result.leveled_schedule, result.diagnostics)
    } else {
        (base_schedule, vec![])
    };

    // Add leveling diagnostics to collector
    for diag in leveling_diagnostics {
        collector.emit(diag);
    }

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

    // Filter diagnostics if --calendars flag is set (clone to get owned Diagnostics)
    let diagnostics: Vec<Diagnostic> = if calendars {
        collector
            .sorted()
            .into_iter()
            .filter(|d| d.code.as_str().starts_with("C"))
            .cloned()
            .collect()
    } else {
        collector.sorted().into_iter().cloned().collect()
    };

    // Emit diagnostics based on format
    let has_errors = match format {
        "json" => {
            // For JSON, collect diagnostics and include in output
            let mut json_emitter = JsonEmitter::new(diag_config);
            for diag in diagnostics.iter().cloned() {
                json_emitter.emit(diag);
            }

            // Format output with diagnostics
            let result = format_json_with_diagnostics(
                &project,
                &schedule,
                show_progress,
                task_ids,
                &json_emitter,
            )?;

            // Write output
            match output {
                Some(path) => {
                    let mut out_file = fs::File::create(path).with_context(|| {
                        format!("Failed to create output file '{}'", path.display())
                    })?;
                    out_file
                        .write_all(result.as_bytes())
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
        "text" => {
            // For text, emit diagnostics to stderr
            let mut term_emitter = TerminalEmitter::new(std::io::stderr(), diag_config);
            for diag in diagnostics {
                term_emitter.emit(diag);
            }

            // Format schedule output (skip if only showing calendar diagnostics)
            if !quiet && !calendars {
                let result =
                    format_text(&project, &schedule, show_progress, task_ids, verbose, width);

                // Write output
                match output {
                    Some(path) => {
                        let mut out_file = fs::File::create(path).with_context(|| {
                            format!("Failed to create output file '{}'", path.display())
                        })?;
                        out_file
                            .write_all(result.as_bytes())
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
        other => {
            // Reject unsupported formats with helpful message
            if other == "xlsx"
                || other == "svg"
                || other == "html"
                || other == "mermaid"
                || other == "plantuml"
            {
                anyhow::bail!(
                    "Format '{}' is not supported by 'schedule' command. Use 'gantt' command instead:\n  \
                    utf8proj gantt {} -f {} -o <output>",
                    other, file.display(), other
                );
            } else {
                anyhow::bail!(
                    "Unknown format '{}'. Supported formats: text, json\n\
                    For graphical outputs (xlsx, svg, html, mermaid, plantuml), use the 'gantt' command.",
                    other
                );
            }
        }
    };

    // Exit with error if there were errors
    if has_errors {
        return Err(anyhow::anyhow!("aborting due to previous error(s)"));
    }

    Ok(())
}

/// Gantt command: generate Gantt chart in various formats
#[allow(clippy::too_many_arguments)]
fn cmd_gantt(
    file: &std::path::Path,
    output: &std::path::Path,
    format: &str,
    task_ids: bool,
    verbose: bool,
    width: usize,
    currency: &str,
    weeks: u32,
    include_calendar: bool,
    include_diagnostics: bool,
    focus: Option<&str>,
    context_depth: usize,
    daily: bool,
    days: u32,
) -> Result<()> {
    use utf8proj_render::DisplayMode;
    // Parse the file
    let project =
        parse_file(file).with_context(|| format!("Failed to parse '{}'", file.display()))?;

    // Schedule the project
    let solver = CpmSolver::new();
    let schedule = solver
        .schedule(&project)
        .with_context(|| "Failed to generate schedule")?;

    // Determine display mode
    let display_mode = if verbose {
        DisplayMode::Verbose
    } else if task_ids {
        DisplayMode::Id
    } else {
        DisplayMode::Name
    };

    // Generate output based on format
    use utf8proj_core::Renderer;

    // Handle xlsx separately (binary output)
    if format.to_lowercase() == "xlsx" {
        let mut renderer = utf8proj_render::ExcelRenderer::new()
            .currency(currency)
            .weeks(weeks);

        // Apply daily granularity if requested
        if daily {
            renderer = renderer.daily().days(days);
            // Use project calendar for working day detection
            if let Some(calendar) = project.calendars.first().cloned() {
                renderer = renderer.with_calendar(calendar);
            }

            // Auto-detect if days parameter is insufficient for full project
            let schedule_end = schedule.project_end;
            let required_days = (schedule_end - project.start).num_days() + 1;
            if required_days > days as i64 {
                // Count tasks that will be omitted (start beyond the window)
                let window_end = project.start + chrono::Duration::days(days as i64 - 1);
                let omitted_tasks: Vec<_> = schedule
                    .tasks
                    .values()
                    .filter(|t| t.start > window_end)
                    .collect();
                let omitted_count = omitted_tasks.len();
                let omitted_hours: f64 = omitted_tasks
                    .iter()
                    .filter_map(|t| {
                        // Get task effort from project
                        find_task_effort(&project.tasks, &t.task_id)
                    })
                    .sum();

                eprintln!();
                eprintln!(
                    "⚠️  Warning: Schedule extends beyond {} days (requires {} days)",
                    days, required_days
                );
                if omitted_count > 0 {
                    eprintln!(
                        "   {} task(s) starting after {} will show 0 hours (~{:.0}h omitted)",
                        omitted_count,
                        window_end.format("%Y-%m-%d"),
                        omitted_hours * 8.0
                    );
                }
                eprintln!(
                    "   Use `--days {}` to see full project timeline",
                    required_days
                );
                eprintln!();
            }
        }

        // Add calendar analysis sheet if requested
        if include_calendar {
            renderer = renderer.with_calendar_analysis();
        }

        // Add diagnostics sheet if requested
        if include_diagnostics || include_calendar {
            // Run project analysis to get diagnostics
            use utf8proj_core::CollectingEmitter;
            use utf8proj_solver::{analyze_project, AnalysisConfig};

            let mut emitter = CollectingEmitter::new();
            let config = AnalysisConfig::default();
            analyze_project(&project, Some(&schedule), &config, &mut emitter);

            let diagnostics: Vec<_> = emitter.diagnostics.into_iter().collect();
            renderer = renderer.with_diagnostics(diagnostics);
        }

        let bytes = renderer
            .render(&project, &schedule)
            .with_context(|| "Failed to render Excel workbook")?;
        fs::write(output, &bytes)
            .with_context(|| format!("Failed to write XLSX to '{}'", output.display()))?;

        let mut features = Vec::new();
        if include_calendar {
            features.push("Calendar Analysis");
        }
        if include_diagnostics {
            features.push("Diagnostics");
        }
        let feature_str = if features.is_empty() {
            String::new()
        } else {
            format!(" with {}", features.join(", "))
        };
        println!(
            "Excel workbook{} written to: {}",
            feature_str,
            output.display()
        );
        return Ok(());
    }

    // Text-based formats
    let content = match format.to_lowercase().as_str() {
        "svg" => {
            let renderer = utf8proj_render::SvgRenderer::new()
                .display_mode(display_mode)
                .label_width(width as u32);
            renderer
                .render(&project, &schedule)
                .with_context(|| "Failed to render SVG Gantt chart")?
        }
        "html" => {
            // HTML format with focus view support
            use utf8proj_render::gantt::{FocusConfig, HtmlGanttRenderer};

            let mut renderer = HtmlGanttRenderer::new();
            renderer.label_width = width as u32;

            // Configure focus view if --focus is provided
            if let Some(focus_pattern) = focus {
                let patterns: Vec<String> = focus_pattern
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
                if !patterns.is_empty() {
                    renderer.focus = Some(FocusConfig::new(patterns, context_depth));
                    eprintln!(
                        "Focus view: showing {} pattern(s), context depth {}",
                        renderer.focus.as_ref().unwrap().focus_patterns.len(),
                        context_depth
                    );
                }
            }

            renderer
                .render(&project, &schedule)
                .with_context(|| "Failed to render HTML Gantt chart")?
        }
        "mermaid" => {
            let renderer = utf8proj_render::MermaidRenderer::new()
                .display_mode(display_mode)
                .label_width(width);
            renderer
                .render(&project, &schedule)
                .with_context(|| "Failed to render Mermaid Gantt chart")?
        }
        "plantuml" => {
            let renderer = utf8proj_render::PlantUmlRenderer::new()
                .display_mode(display_mode)
                .label_width(width);
            renderer
                .render(&project, &schedule)
                .with_context(|| "Failed to render PlantUML Gantt chart")?
        }
        _ => {
            anyhow::bail!(
                "Unknown format '{}'. Supported formats: svg, html, mermaid, plantuml, xlsx",
                format
            );
        }
    };

    // Write to file
    fs::write(output, &content).with_context(|| {
        format!(
            "Failed to write {} to '{}'",
            format.to_uppercase(),
            output.display()
        )
    })?;

    println!("Gantt chart ({}) written to: {}", format, output.display());
    Ok(())
}

/// Count tasks recursively (including nested tasks)
fn count_tasks(tasks: &[utf8proj_core::Task]) -> usize {
    tasks.iter().map(|t| 1 + count_tasks(&t.children)).sum()
}

/// Format schedule as text table
fn format_text(
    project: &utf8proj_core::Project,
    schedule: &utf8proj_core::Schedule,
    show_progress: bool,
    task_ids: bool,
    verbose: bool,
    width: usize,
) -> String {
    let mut output = String::new();

    // Header
    output.push_str(&format!("Project: {}\n", project.name));
    output.push_str(&format!("Start: {}\n", project.start));
    output.push_str(&format!("End: {}\n", schedule.project_end));
    output.push_str(&format!(
        "Duration: {} days\n",
        schedule.project_duration.as_days()
    ));

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
        schedule.spi, spi_status, spi_icon, schedule.earned_value, schedule.planned_value
    ));

    // Cost range (RFC-0004: Progressive Resource Refinement)
    if let Some(ref cost_range) = schedule.total_cost_range {
        let spread_pct = cost_range.spread_percent();
        let cost_status = if spread_pct < 10.0 {
            ("narrow", "🟢")
        } else if spread_pct < 50.0 {
            ("moderate", "🟡")
        } else {
            ("wide", "🔴")
        };
        output.push_str(&format!(
            "Cost: {} {} (min: {} / expected: {} / max: {}) {}\n",
            cost_range.expected,
            cost_range.currency,
            cost_range.min,
            cost_range.expected,
            cost_range.max,
            cost_status.1
        ));
        if spread_pct > 0.0 {
            output.push_str(&format!(
                "      spread: {:.1}% ({} range)\n",
                spread_pct, cost_status.0
            ));
        }
    }
    output.push('\n');

    // Critical path
    if !schedule.critical_path.is_empty() {
        output.push_str("Critical Path: ");
        output.push_str(&schedule.critical_path.join(" -> "));
        output.push_str("\n\n");
    }

    if show_progress {
        // Progress-aware output format with variance
        // Other columns: %Done(7) + Status(15) + Start(13) + Finish(13) + Remain(9) + Variance(9) + Critical(8) = 74
        let sep_width = width + 74;
        output.push_str(&format!(
            "{:<width$} {:>6} {:<14} {:<12} {:<12} {:>8} {:>8} {}\n",
            "Task",
            "%Done",
            "Status",
            "Start",
            "Finish",
            "Remain",
            "Variance",
            "Critical",
            width = width
        ));
        output.push_str(&format!("{}\n", "-".repeat(sep_width)));

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
            let display_name = if task_ids {
                task.task_id.clone()
            } else if verbose {
                let name = get_task_display_name(&project.tasks, &task.task_id);
                format!("[{}] {}", task.task_id, name)
            } else {
                get_task_display_name(&project.tasks, &task.task_id)
            };
            output.push_str(&format!(
                "{:<width$} {:>5}% {:<14} {:<12} {:<12} {:>6}d {:>8} {}\n",
                truncate(&display_name, width),
                task.percent_complete,
                format!("{}", task.status),
                task.forecast_start.format("%Y-%m-%d"),
                task.forecast_finish.format("%Y-%m-%d"),
                task.remaining_duration.as_days() as i64,
                variance_str,
                critical,
                width = width
            ));
        }
    } else {
        // Standard output format
        // Other columns: Start(13) + Finish(13) + Duration(9) + Slack(9) + Critical(8) = 52
        let sep_width = width + 52;
        output.push_str(&format!(
            "{:<width$} {:<12} {:<12} {:>8} {:>8} {}\n",
            "Task",
            "Start",
            "Finish",
            "Duration",
            "Slack",
            "Critical",
            width = width
        ));
        output.push_str(&format!("{}\n", "-".repeat(sep_width)));

        // Sort tasks by start date
        let mut tasks: Vec<_> = schedule.tasks.values().collect();
        tasks.sort_by_key(|t| t.start);

        // Task rows
        for task in tasks {
            let critical = if task.is_critical { "*" } else { "" };
            let display_name = if task_ids {
                task.task_id.clone()
            } else if verbose {
                let name = get_task_display_name(&project.tasks, &task.task_id);
                format!("[{}] {}", task.task_id, name)
            } else {
                get_task_display_name(&project.tasks, &task.task_id)
            };
            output.push_str(&format!(
                "{:<width$} {:<12} {:<12} {:>6}d {:>6}d {}\n",
                truncate(&display_name, width),
                task.start.format("%Y-%m-%d"),
                task.finish.format("%Y-%m-%d"),
                task.duration.as_days() as i64,
                task.slack.as_days() as i64,
                critical,
                width = width
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
    _task_ids: bool, // JSON always includes both id and name
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

    serde_json::to_string_pretty(&summary).with_context(|| "Failed to serialize schedule to JSON")
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
/// Task IDs can be hierarchical like "parent.child.grandchild", where each level
/// of the tree has simple IDs like "parent", "child", "grandchild".
fn find_task_info(tasks: &[utf8proj_core::Task], id: &str) -> Option<(String, Option<String>)> {
    for task in tasks {
        if task.id == id {
            return Some((task.name.clone(), task.summary.clone()));
        }
        // For nested tasks, the ID might be like "parent.child.grandchild"
        // Strip the current level's prefix before searching children
        let prefix = format!("{}.", task.id);
        if id.starts_with(&prefix) {
            let child_id = &id[prefix.len()..]; // Strip parent prefix
            if let Some(result) = find_task_info(&task.children, child_id) {
                return Some(result);
            }
        }
    }
    None
}

/// Find task effort in person-days (recursively searches nested tasks)
fn find_task_effort(tasks: &[utf8proj_core::Task], id: &str) -> Option<f64> {
    for task in tasks {
        if task.id == id {
            return task.effort.map(|d| d.as_days() as f64);
        }
        // For nested tasks, the ID might be like "parent.child.grandchild"
        let prefix = format!("{}.", task.id);
        if id.starts_with(&prefix) {
            let child_id = &id[prefix.len()..];
            if let Some(result) = find_task_effort(&task.children, child_id) {
                return Some(result);
            }
        }
    }
    None
}

/// Get the display name for a task using fallback: name (quoted string) → id
/// The `name` field contains the human-readable task name from the DSL quoted string.
/// The `summary` field is supplementary info, not the primary display name.
fn get_task_display_name(tasks: &[utf8proj_core::Task], id: &str) -> String {
    if let Some((name, _summary)) = find_task_info(tasks, id) {
        // Use name (the quoted string) if it's meaningful
        if !name.is_empty() && name != id {
            return name;
        }
    }
    id.to_string()
}

/// Fix container dependencies command: propagate container deps to children
/// Uses text-based patching to preserve comments and formatting.
fn cmd_fix_container_deps(
    file: &std::path::Path,
    output: Option<&std::path::Path>,
    in_place: bool,
) -> Result<()> {
    // Validate arguments
    if in_place && output.is_some() {
        anyhow::bail!("Cannot specify both --in-place and --output");
    }

    // Read the original file content
    let original_content =
        fs::read_to_string(file).with_context(|| format!("Failed to read '{}'", file.display()))?;

    // Parse the file to analyze what needs fixing
    let project =
        parse_file(file).with_context(|| format!("Failed to parse '{}'", file.display()))?;

    // Count W014 diagnostics before fix
    let mut collector = utf8proj_core::CollectingEmitter::new();
    let analysis_config = AnalysisConfig::new().with_file(file);
    analyze_project(&project, None, &analysis_config, &mut collector);
    let w014_before = collector
        .diagnostics
        .iter()
        .filter(|d| d.code == DiagnosticCode::W014ContainerDependency)
        .count();

    // Collect fixes needed: map from task_id to dependencies to add
    let fixes = collect_container_dep_fixes(&project.tasks, &[]);

    // Apply text-based patches
    let (output_content, fixed_count) = apply_dependency_patches(&original_content, &fixes)?;

    // Write output
    if in_place {
        fs::write(file, &output_content)
            .with_context(|| format!("Failed to write '{}'", file.display()))?;
        eprintln!(
            "Fixed {} container dependencies in '{}'",
            fixed_count,
            file.display()
        );
    } else if let Some(out_path) = output {
        fs::write(out_path, &output_content)
            .with_context(|| format!("Failed to write '{}'", out_path.display()))?;
        eprintln!(
            "Fixed {} container dependencies, written to '{}'",
            fixed_count,
            out_path.display()
        );
    } else {
        // Write to stdout
        println!("{}", output_content);
        eprintln!("Fixed {} container dependencies", fixed_count);
    }

    // Report results
    if fixed_count > 0 {
        eprintln!("  W014 warnings before: {}", w014_before);
        eprintln!("  Dependencies added: {}", fixed_count);
        eprintln!();
        eprintln!("Run 'utf8proj check' to verify the fix.");
    } else {
        eprintln!("No container dependency issues found.");
    }

    Ok(())
}

/// Dependency to add to a task
#[derive(Debug, Clone)]
struct DepToAdd {
    predecessor: String,
    dep_type: utf8proj_core::DependencyType,
    lag: Option<utf8proj_core::Duration>,
}

impl DepToAdd {
    fn to_string(&self) -> String {
        let mut s = self.predecessor.clone();
        match self.dep_type {
            utf8proj_core::DependencyType::StartToStart => s.push_str(" SS"),
            utf8proj_core::DependencyType::StartToFinish => s.push_str(" SF"),
            utf8proj_core::DependencyType::FinishToFinish => s.push_str(" FF"),
            utf8proj_core::DependencyType::FinishToStart => {} // Default
        }
        if let Some(ref lag) = self.lag {
            let days = lag.as_days();
            if days >= 0.0 {
                s.push_str(&format!(" +{}d", days as i64));
            } else {
                s.push_str(&format!(" {}d", days as i64));
            }
        }
        s
    }
}

/// Collect all fixes needed: returns map of task_id -> dependencies to add
fn collect_container_dep_fixes(
    tasks: &[utf8proj_core::Task],
    parent_deps: &[DepToAdd],
) -> std::collections::HashMap<String, Vec<DepToAdd>> {
    let mut fixes = std::collections::HashMap::new();

    for task in tasks {
        // Combine parent deps with this task's deps for children
        let mut deps_for_children: Vec<DepToAdd> = parent_deps.to_vec();
        for dep in &task.depends {
            deps_for_children.push(DepToAdd {
                predecessor: dep.predecessor.clone(),
                dep_type: dep.dep_type,
                lag: dep.lag,
            });
        }

        if !task.children.is_empty() {
            // This is a container - check each child
            for child in &task.children {
                let child_deps: std::collections::HashSet<_> = child
                    .depends
                    .iter()
                    .map(|d| d.predecessor.clone())
                    .collect();

                // Check if child needs any of the container's dependencies
                let has_container_dep = deps_for_children
                    .iter()
                    .any(|d| child_deps.contains(&d.predecessor));

                if !has_container_dep && !deps_for_children.is_empty() {
                    // Child needs container dependencies
                    let deps_to_add: Vec<_> = deps_for_children
                        .iter()
                        .filter(|d| !child_deps.contains(&d.predecessor))
                        .cloned()
                        .collect();
                    if !deps_to_add.is_empty() {
                        fixes.insert(child.id.clone(), deps_to_add);
                    }
                }
            }

            // Recurse into children
            let child_fixes = collect_container_dep_fixes(&task.children, &deps_for_children);
            fixes.extend(child_fixes);
        }
    }

    fixes
}

/// Apply text-based patches to add dependencies
fn apply_dependency_patches(
    content: &str,
    fixes: &std::collections::HashMap<String, Vec<DepToAdd>>,
) -> Result<(String, usize)> {
    use regex::Regex;

    let mut result = content.to_string();
    let mut total_fixed = 0;

    // Process each task that needs fixes
    for (task_id, deps_to_add) in fixes {
        // Find the task block: task task_id "name" { or task task_id { or milestone task_id ...
        // Use word boundary to avoid matching task_id as substring of another task
        let task_pattern = format!(
            r#"(?m)^(\s*)((?:task|milestone)\s+{}\s+(?:"[^"]*"\s*)?\{{)"#,
            regex::escape(task_id)
        );
        let task_re = Regex::new(&task_pattern)?;

        if let Some(caps) = task_re.captures(&result) {
            let indent = caps.get(1).map(|m| m.as_str()).unwrap_or("");
            let task_match = caps.get(0).unwrap();
            let task_start = task_match.end();

            // Find if there's already a depends: line in this task block
            // We need to find content between { and the next }
            // But we need to handle nested tasks, so we'll look for depends: before any nested task
            let remaining = &result[task_start..];

            // Find the position of the first nested task or closing brace
            let nested_task_re = Regex::new(r"(?m)^\s*task\s+")?;
            let first_nested = nested_task_re.find(remaining).map(|m| m.start());
            let first_brace = remaining.find('}');

            let search_end = match (first_nested, first_brace) {
                (Some(n), Some(b)) => n.min(b),
                (Some(n), None) => n,
                (None, Some(b)) => b,
                (None, None) => remaining.len(),
            };

            let task_body = &remaining[..search_end];

            // Check for existing depends: line
            let depends_re = Regex::new(r"(?m)^(\s*)(depends:\s*)(.*)$")?;

            // Format new dependencies
            let new_deps: Vec<String> = deps_to_add.iter().map(|d| d.to_string()).collect();

            if let Some(dep_caps) = depends_re.captures(task_body) {
                // Append to existing depends: line
                let dep_match = dep_caps.get(0).unwrap();
                let existing_deps = dep_caps.get(3).map(|m| m.as_str()).unwrap_or("");
                let abs_pos = task_start + dep_match.start();
                let abs_end = task_start + dep_match.end();

                // Build new depends line
                let dep_indent = dep_caps.get(1).map(|m| m.as_str()).unwrap_or("");
                let new_line = if existing_deps.trim().is_empty() {
                    format!("{}depends: {}", dep_indent, new_deps.join(", "))
                } else {
                    format!(
                        "{}depends: {}, {}",
                        dep_indent,
                        existing_deps.trim(),
                        new_deps.join(", ")
                    )
                };

                result = format!("{}{}{}", &result[..abs_pos], new_line, &result[abs_end..]);
            } else {
                // Insert new depends: line after the opening brace
                // Find position right after { and any following whitespace/newline
                let inner_indent = format!("{}    ", indent);
                let new_line = format!("\n{}depends: {}", inner_indent, new_deps.join(", "));

                result = format!(
                    "{}{}{}",
                    &result[..task_start],
                    new_line,
                    &result[task_start..]
                );
            }

            total_fixed += deps_to_add.len();
        }
    }

    Ok((result, total_fixed))
}

/// Serialize a Project to .proj format
fn serialize_project(project: &utf8proj_core::Project) -> String {
    let mut output = String::new();

    // Project header - use id only if it's a valid simple identifier different from name
    let id_is_valid = project.id.chars().all(|c| c.is_alphanumeric() || c == '_');
    if id_is_valid && project.id != project.name && !project.id.is_empty() {
        output.push_str(&format!("project {} \"{}\" {{\n", project.id, project.name));
    } else {
        output.push_str(&format!("project \"{}\" {{\n", project.name));
    }
    output.push_str(&format!("    start: {}\n", project.start));
    if let Some(status_date) = project.status_date {
        output.push_str(&format!("    status_date: {}\n", status_date));
    }
    output.push_str("}\n\n");

    // Calendars
    for calendar in &project.calendars {
        output.push_str(&format!("calendar \"{}\" {{\n", calendar.id));
        if !calendar.working_days.is_empty() {
            // Working days: 0 = Sunday, 1 = Monday, ..., 6 = Saturday
            // Use mon-fri shorthand if it's standard working days (1-5)
            let is_standard = calendar.working_days.len() == 5
                && calendar.working_days.contains(&1)  // Mon
                && calendar.working_days.contains(&2)  // Tue
                && calendar.working_days.contains(&3)  // Wed
                && calendar.working_days.contains(&4)  // Thu
                && calendar.working_days.contains(&5); // Fri
            if is_standard {
                output.push_str("    working_days: mon-fri\n");
            } else {
                let days: Vec<_> = calendar
                    .working_days
                    .iter()
                    .map(|d| match d {
                        0 => "sun",
                        1 => "mon",
                        2 => "tue",
                        3 => "wed",
                        4 => "thu",
                        5 => "fri",
                        6 => "sat",
                        _ => "?",
                    })
                    .collect();
                output.push_str(&format!("    working_days: {}\n", days.join(", ")));
            }
        }
        // Working hours
        if !calendar.working_hours.is_empty() {
            let hours: Vec<_> = calendar
                .working_hours
                .iter()
                .map(|tr| {
                    let start_h = tr.start / 60;
                    let start_m = tr.start % 60;
                    let end_h = tr.end / 60;
                    let end_m = tr.end % 60;
                    format!("{:02}:{:02}-{:02}:{:02}", start_h, start_m, end_h, end_m)
                })
                .collect();
            output.push_str(&format!("    working_hours: {}\n", hours.join(", ")));
        }
        for holiday in &calendar.holidays {
            if holiday.start == holiday.end {
                output.push_str(&format!(
                    "    holiday \"{}\" {}\n",
                    holiday.name, holiday.start
                ));
            } else {
                output.push_str(&format!(
                    "    holiday \"{}\" {}..{}\n",
                    holiday.name, holiday.start, holiday.end
                ));
            }
        }
        output.push_str("}\n\n");
    }

    // Resources
    for resource in &project.resources {
        output.push_str(&format!(
            "resource {} \"{}\" {{\n",
            resource.id, resource.name
        ));
        if let Some(ref cal) = resource.calendar {
            output.push_str(&format!("    calendar: {}\n", cal));
        }
        if resource.capacity != 1.0 {
            output.push_str(&format!(
                "    capacity: {}%\n",
                (resource.capacity * 100.0) as i32
            ));
        }
        output.push_str("}\n\n");
    }

    // Tasks (recursive)
    for task in &project.tasks {
        serialize_task(&mut output, task, 0);
    }

    output
}

/// Serialize a task (recursive for nested tasks)
fn serialize_task(output: &mut String, task: &utf8proj_core::Task, indent: usize) {
    let indent_str = "    ".repeat(indent);

    // Task declaration
    if task.name != task.id {
        output.push_str(&format!(
            "{}task {} \"{}\" {{\n",
            indent_str, task.id, task.name
        ));
    } else {
        output.push_str(&format!("{}task {} {{\n", indent_str, task.id));
    }

    let inner_indent = "    ".repeat(indent + 1);

    // Duration
    if let Some(ref dur) = task.duration {
        output.push_str(&format!("{}duration: {}d\n", inner_indent, dur.as_days()));
    }
    // Effort (can coexist with duration)
    if let Some(ref eff) = task.effort {
        output.push_str(&format!("{}effort: {}d\n", inner_indent, eff.as_days()));
    }

    // Milestone
    if task.milestone {
        output.push_str(&format!("{}milestone: true\n", inner_indent));
    }

    // Dependencies
    if !task.depends.is_empty() {
        let deps: Vec<_> = task
            .depends
            .iter()
            .map(|d| {
                let mut dep_str = d.predecessor.clone();
                // Add dependency type suffix (FS is default, no suffix needed)
                match d.dep_type {
                    utf8proj_core::DependencyType::StartToStart => dep_str.push_str(" SS"),
                    utf8proj_core::DependencyType::StartToFinish => dep_str.push_str(" SF"),
                    utf8proj_core::DependencyType::FinishToFinish => dep_str.push_str(" FF"),
                    utf8proj_core::DependencyType::FinishToStart => {} // Default, no suffix
                }
                // Add lag
                if let Some(ref lag) = d.lag {
                    let days = lag.as_days();
                    if days >= 0.0 {
                        dep_str.push_str(&format!(" +{}d", days as i64));
                    } else {
                        dep_str.push_str(&format!(" {}d", days as i64));
                    }
                }
                dep_str
            })
            .collect();
        output.push_str(&format!("{}depends: {}\n", inner_indent, deps.join(", ")));
    }

    // Assignments
    if !task.assigned.is_empty() {
        let assigns: Vec<_> = task
            .assigned
            .iter()
            .map(|a| {
                let percentage = (a.units * 100.0) as i32;
                if percentage != 100 {
                    format!("{}@{}%", a.resource_id, percentage)
                } else {
                    a.resource_id.clone()
                }
            })
            .collect();
        output.push_str(&format!("{}assign: {}\n", inner_indent, assigns.join(", ")));
    }

    // Constraints
    for constraint in &task.constraints {
        match constraint {
            utf8proj_core::TaskConstraint::MustStartOn(date) => {
                output.push_str(&format!("{}must_start_on: {}\n", inner_indent, date));
            }
            utf8proj_core::TaskConstraint::MustFinishOn(date) => {
                output.push_str(&format!("{}must_finish_on: {}\n", inner_indent, date));
            }
            utf8proj_core::TaskConstraint::StartNoEarlierThan(date) => {
                output.push_str(&format!(
                    "{}start_no_earlier_than: {}\n",
                    inner_indent, date
                ));
            }
            utf8proj_core::TaskConstraint::StartNoLaterThan(date) => {
                output.push_str(&format!("{}start_no_later_than: {}\n", inner_indent, date));
            }
            utf8proj_core::TaskConstraint::FinishNoEarlierThan(date) => {
                output.push_str(&format!(
                    "{}finish_no_earlier_than: {}\n",
                    inner_indent, date
                ));
            }
            utf8proj_core::TaskConstraint::FinishNoLaterThan(date) => {
                output.push_str(&format!("{}finish_no_later_than: {}\n", inner_indent, date));
            }
        }
    }

    // Summary (optional short display name)
    if let Some(ref summary) = task.summary {
        output.push_str(&format!("{}summary: \"{}\"\n", inner_indent, summary));
    }

    // Completion
    if let Some(complete) = task.complete {
        output.push_str(&format!("{}complete: {}%\n", inner_indent, complete as i32));
    }

    // Remaining duration (explicit override)
    if let Some(ref remaining) = task.explicit_remaining {
        output.push_str(&format!(
            "{}remaining: {}d\n",
            inner_indent,
            remaining.as_days()
        ));
    }

    // Priority (only if non-default)
    if task.priority != 500 {
        output.push_str(&format!("{}priority: {}\n", inner_indent, task.priority));
    }

    // Note (from attributes)
    if let Some(note) = task.attributes.get("note") {
        output.push_str(&format!("{}note: \"{}\"\n", inner_indent, note));
    }

    // Tags (from attributes)
    if let Some(tags) = task.attributes.get("tags") {
        output.push_str(&format!("{}tag: {}\n", inner_indent, tags));
    }

    // Cost (from attributes)
    if let Some(cost) = task.attributes.get("cost") {
        output.push_str(&format!("{}cost: {}\n", inner_indent, cost));
    }

    // Payment (from attributes)
    if let Some(payment) = task.attributes.get("payment") {
        output.push_str(&format!("{}payment: {}\n", inner_indent, payment));
    }

    // Children (recursive)
    for child in &task.children {
        output.push('\n');
        serialize_task(output, child, indent + 1);
    }

    output.push_str(&format!("{}}}\n", indent_str));
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
    println!(
        "  Resource Leveling: {}",
        if leveling { "enabled" } else { "disabled" }
    );
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
        println!(
            "Running single benchmark with {} tasks, {} resources...",
            tasks, resources
        );
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

// ============================================================================
// Classify Command (RFC-0011)
// ============================================================================

fn cmd_classify(file: &std::path::Path, by: &str) -> Result<()> {
    use utf8proj_core::{group_by, Classifier, StatusClassifier};

    // Parse project file
    let project = parse_file(file)?;

    // Schedule the project (classifiers may need schedule data)
    let schedule = utf8proj_solver::CpmSolver::new().schedule(&project)?;

    // Select classifier
    let classifier: Box<dyn Classifier> = match by.to_lowercase().as_str() {
        "status" => Box::new(StatusClassifier),
        other => {
            eprintln!("error: unknown classifier '{}'. Available: status", other);
            std::process::exit(1);
        }
    };

    // Group tasks
    let groups = group_by(&project, &schedule, classifier.as_ref());

    // Output results
    println!("{}:", classifier.name());
    for (category, tasks) in groups {
        let task_ids: Vec<_> = tasks.iter().map(|t| t.id.as_str()).collect();
        if task_ids.is_empty() {
            continue;
        }
        println!("  {}: {}", category, task_ids.join(", "));
    }

    Ok(())
}

// ============================================================================
// Init Command
// ============================================================================

/// Initialize a new project file with a working example
fn cmd_init(name: &str, output_dir: Option<&std::path::Path>) -> Result<()> {
    use chrono::Local;

    // Determine output directory
    let dir = output_dir
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| ".".into()));

    // Sanitize project name for filename (replace spaces/special chars with underscores)
    let filename: String = name
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect();
    let filepath = dir.join(format!("{}.proj", filename));

    // Check if file already exists
    if filepath.exists() {
        anyhow::bail!(
            "File '{}' already exists. Use a different name or delete the existing file.",
            filepath.display()
        );
    }

    // Get today's date for the project start
    let today = Local::now().format("%Y-%m-%d");

    // Generate project template
    let template = format!(
        r#"# {name}
#
# This is a utf8proj project file. Edit this file to define your project schedule.
#
# Quick reference:
#   - Tasks have duration (calendar time) or effort (work time)
#   - Dependencies: FS (finish-to-start, default), SS, FF, SF
#   - Lag: +2d (delay), -1d (lead)
#   - Resources: assign tasks to named resources
#
# Commands:
#   utf8proj schedule {filename}.proj       # Show schedule
#   utf8proj gantt {filename}.proj -o gantt.html -f html  # Visual Gantt chart
#   utf8proj check {filename}.proj          # Validate only
#
# Docs: https://github.com/alanbld/utf8proj

project "{name}" {{
    start: {today}
}}

# Define your resources
resource dev "Developer" {{
    rate: 800/day
}}

resource design "Designer" {{
    rate: 600/day
}}

# Define your tasks
# Tip: Tasks with 'depends:' create a schedule chain

task planning "Planning" {{
    duration: 3d
    assign: dev
}}

task design "Design Phase" {{
    duration: 5d
    depends: planning
    assign: design
}}

task development "Development" {{
    duration: 10d
    depends: design
    assign: dev
}}

task testing "Testing" {{
    duration: 3d
    depends: development
    assign: dev
}}

milestone launch "Project Launch" {{
    depends: testing
}}

# Next steps:
# 1. Edit the tasks above to match your project
# 2. Run: utf8proj schedule {filename}.proj
# 3. Generate Gantt: utf8proj gantt {filename}.proj -o gantt.html -f html
"#,
        name = name,
        filename = filename,
        today = today
    );

    // Create parent directory if needed
    if let Some(parent) = filepath.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create directory '{}'", parent.display()))?;
        }
    }

    // Write the file
    fs::write(&filepath, &template)
        .with_context(|| format!("Failed to write '{}'", filepath.display()))?;

    println!("Created: {}", filepath.display());
    println!();
    println!("Next steps:");
    println!("  1. Edit {} to define your project", filepath.display());
    println!("  2. Run: utf8proj schedule {}", filepath.display());
    println!(
        "  3. Generate Gantt: utf8proj gantt {} -o gantt.html -f html",
        filepath.display()
    );

    Ok(())
}
