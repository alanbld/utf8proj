//! utf8proj CLI - Project Scheduling Engine
//!
//! Command-line interface for parsing, scheduling, and rendering projects.

use anyhow::Result;
use clap::{Parser, Subcommand};
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

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

        /// Output format (svg, text, json)
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
        Some(Commands::Check { file }) => {
            println!("Checking: {}", file.display());
            // TODO: Implement check command
        }
        Some(Commands::Schedule { file, format, output }) => {
            println!("Scheduling: {} (format: {})", file.display(), format);
            if let Some(out) = output {
                println!("Output: {}", out.display());
            }
            // TODO: Implement schedule command
        }
        Some(Commands::Gantt { file, output }) => {
            println!("Generating Gantt chart: {} -> {}", file.display(), output.display());
            // TODO: Implement gantt command
        }
        None => {
            println!("utf8proj - Project Scheduling Engine");
            println!("Run with --help for usage information");
        }
    }

    Ok(())
}
