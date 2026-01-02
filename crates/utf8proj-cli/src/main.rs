//! utf8proj CLI - Project Scheduling Engine
//!
//! Command-line interface for parsing, scheduling, and rendering projects.

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::fs;
use std::io::Write;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

use utf8proj_core::Scheduler;
use utf8proj_parser::parse_file;
use utf8proj_solver::CpmSolver;

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
    /// Parse and validate a project file
    Check {
        /// Input file path
        #[arg(value_name = "FILE")]
        file: std::path::PathBuf,
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
}

fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Check { file }) => cmd_check(&file),
        Some(Commands::Schedule { file, format, output }) => {
            cmd_schedule(&file, &format, output.as_deref())
        }
        Some(Commands::Gantt { file, output }) => cmd_gantt(&file, &output),
        None => {
            println!("utf8proj - Project Scheduling Engine");
            println!();
            println!("Usage: utf8proj <COMMAND>");
            println!();
            println!("Commands:");
            println!("  check     Parse and validate a project file");
            println!("  schedule  Schedule a project and output results");
            println!("  gantt     Generate a Gantt chart (SVG)");
            println!();
            println!("Run 'utf8proj --help' for more information");
            Ok(())
        }
    }
}

/// Check command: parse and validate a project file
fn cmd_check(file: &std::path::Path) -> Result<()> {
    println!("Checking: {}", file.display());

    // Parse the file
    let project = parse_file(file)
        .with_context(|| format!("Failed to parse '{}'", file.display()))?;

    println!("  Project: {}", project.name);
    println!("  Start: {}", project.start);
    if let Some(end) = project.end {
        println!("  End: {}", end);
    }
    println!("  Tasks: {}", count_tasks(&project.tasks));
    println!("  Resources: {}", project.resources.len());

    // Check feasibility
    let solver = CpmSolver::new();
    let feasibility = solver.is_feasible(&project);

    if feasibility.feasible {
        println!("  Status: OK - No circular dependencies detected");
    } else {
        println!("  Status: ERRORS FOUND");
        for conflict in &feasibility.conflicts {
            println!("    - {}: {}",
                format!("{:?}", conflict.conflict_type),
                conflict.description
            );
        }
        return Err(anyhow::anyhow!("Project has feasibility issues"));
    }

    Ok(())
}

/// Schedule command: parse, schedule, and output results
fn cmd_schedule(
    file: &std::path::Path,
    format: &str,
    output: Option<&std::path::Path>,
) -> Result<()> {
    // Parse the file
    let project = parse_file(file)
        .with_context(|| format!("Failed to parse '{}'", file.display()))?;

    // Check feasibility first
    let solver = CpmSolver::new();
    let feasibility = solver.is_feasible(&project);

    if !feasibility.feasible {
        eprintln!("Error: Project is not feasible");
        for conflict in &feasibility.conflicts {
            eprintln!("  - {}: {}",
                format!("{:?}", conflict.conflict_type),
                conflict.description
            );
        }
        return Err(anyhow::anyhow!("Cannot schedule infeasible project"));
    }

    // Schedule the project
    let schedule = solver.schedule(&project)
        .with_context(|| "Failed to generate schedule")?;

    // Format output
    let result = match format {
        "json" => format_json(&project, &schedule)?,
        "text" | _ => format_text(&project, &schedule),
    };

    // Write output
    match output {
        Some(path) => {
            let mut file = fs::File::create(path)
                .with_context(|| format!("Failed to create output file '{}'", path.display()))?;
            file.write_all(result.as_bytes())
                .with_context(|| "Failed to write output")?;
            println!("Schedule written to: {}", path.display());
        }
        None => {
            println!("{}", result);
        }
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
fn format_text(project: &utf8proj_core::Project, schedule: &utf8proj_core::Schedule) -> String {
    let mut output = String::new();

    // Header
    output.push_str(&format!("Project: {}\n", project.name));
    output.push_str(&format!("Start: {}\n", project.start));
    output.push_str(&format!("End: {}\n", schedule.project_end));
    output.push_str(&format!("Duration: {} days\n", schedule.project_duration.as_days()));
    output.push('\n');

    // Critical path
    if !schedule.critical_path.is_empty() {
        output.push_str("Critical Path: ");
        output.push_str(&schedule.critical_path.join(" -> "));
        output.push_str("\n\n");
    }

    // Task table header
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
        output.push_str(&format!(
            "{:<20} {:<12} {:<12} {:>6}d {:>6}d {}\n",
            truncate(&task.task_id, 20),
            task.start.format("%Y-%m-%d"),
            task.finish.format("%Y-%m-%d"),
            task.duration.as_days() as i64,
            task.slack.as_days() as i64,
            critical
        ));
    }

    output
}

/// Format schedule as JSON
fn format_json(
    project: &utf8proj_core::Project,
    schedule: &utf8proj_core::Schedule,
) -> Result<String> {
    // Create a summary structure for JSON output
    let summary = serde_json::json!({
        "project": {
            "name": project.name,
            "start": project.start.to_string(),
            "end": schedule.project_end.to_string(),
            "duration_days": schedule.project_duration.as_days(),
        },
        "critical_path": schedule.critical_path,
        "tasks": schedule.tasks.values().map(|t| {
            serde_json::json!({
                "id": t.task_id,
                "start": t.start.to_string(),
                "finish": t.finish.to_string(),
                "duration_days": t.duration.as_days(),
                "slack_days": t.slack.as_days(),
                "is_critical": t.is_critical,
                "early_start": t.early_start.to_string(),
                "early_finish": t.early_finish.to_string(),
                "late_start": t.late_start.to_string(),
                "late_finish": t.late_finish.to_string(),
            })
        }).collect::<Vec<_>>(),
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
