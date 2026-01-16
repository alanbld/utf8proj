<div align="center">
  <img src="docs/logos/logo.svg" alt="utf8proj" width="400">

  # utf8proj

  **Explainable Project Scheduling for Text-Based Workflows**
</div>

[![Build](https://github.com/alanbld/utf8proj/actions/workflows/ci.yml/badge.svg)](https://github.com/alanbld/utf8proj/actions)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org/)

utf8proj is a **deterministic, explainable project scheduling engine** that transforms plain text project definitions into auditable schedules. Built on CPM (Critical Path Method) with calendar awareness, it provides transparency where traditional tools offer only black-box optimization.

**[Try the Interactive Demo](https://alanbld.github.io/utf8proj/)** - No installation required, runs in your browser via WebAssembly.

## Why utf8proj?

| Traditional Tools | utf8proj |
|-------------------|----------|
| "The schedule changed" | Every change has a **traceable reason** |
| Black-box optimization | **Deterministic** CPM + explicit leveling |
| Binary formats | **Git-friendly** text files (`.proj`) |
| Silent auto-correction | **Diagnostics explain**, don't fix |
| Methodology enforcement | **Philosophy neutral** — describe reality |

### Core Philosophy

utf8proj follows a **"describe, don't prescribe"** approach. It explains *why* schedules look the way they do, without enforcing specific project management methodologies or silently "fixing" your model.

Read our [Explainability Manifesto](docs/EXPLAINABILITY.md) for the full philosophy.

## Key Capabilities

### Scheduling & Analysis
- **CPM Scheduling** — Critical path calculation with FS/SS/FF/SF dependencies and lag
- **Deterministic Resource Leveling** — Opt-in conflict resolution with complete audit trail
- **Calendar-Aware** — Working days, weekends, holidays with impact quantification
- **Progress-Aware Scheduling** — Status date resolution, remaining duration calculation, forecasts
- **Earned Value Analysis** — Variance detection (SPI), baseline vs forecast comparison
- **Hierarchical Tasks** — Nested task structures with automatic container date derivation

### Explainability & Diagnostics
- **40+ Diagnostic Codes** across multiple categories:
  - **E***: Errors (circular dependencies, infeasible constraints)
  - **W***: Warnings (overallocation, wide cost ranges)
  - **H***: Hints (unused profiles, unconstrained tasks)
  - **I***: Info (utilization summaries, project status, earned value)
  - **C***: Calendar issues (C001-C023)
  - **L***: Leveling decisions (L001-L004)
  - **P***: Progress tracking (P005-P006 remaining vs complete conflicts)
- **CalendarImpact Analysis** — Working days vs calendar days per task
- **Diagnostic→Task Linking** — Trace every diagnostic to affected tasks

### Tooling & Integration
- **LSP Support** — IDE integration with hover explanations, go-to-definition, find-references
- **WASM Playground** — Interactive browser scheduling with live Gantt preview
- **Excel Export** — Formula-driven workbooks with auto-fit, dependency cascading, Calendar Analysis
- **Focus View** — Filter Gantt charts by task prefix with configurable context depth
- **Multiple Formats** — HTML, SVG, Mermaid, PlantUML, XLSX

## Quick Start

### Installation

```bash
# From source
git clone https://github.com/alanbld/utf8proj.git
cd utf8proj
cargo install --path crates/utf8proj-cli
```

### Create a Project File

```proj
# project.proj

project "Website Redesign" {
    start: 2025-02-01
    currency: USD
}

calendar "standard" {
    working_days: mon-fri
    working_hours: 09:00-17:00
}

resource designer "UI Designer" { rate: 750/day }
resource developer "Developer" { rate: 850/day }

task design "Design Phase" {
    task wireframes "Wireframes" { effort: 3d, assign: designer }
    task mockups "Mockups" { effort: 5d, assign: designer, depends: wireframes }
}

task development "Development" {
    depends: design

    task frontend "Frontend" { effort: 10d, assign: developer }
    task backend "Backend" { effort: 8d, assign: developer }
}

milestone launch "Launch" {
    depends: development
}
```

### Generate Schedule

```bash
# Validate project (fast, no scheduling)
utf8proj check project.proj
utf8proj check --strict project.proj    # Warnings become errors

# Compute and display schedule
utf8proj schedule project.proj
utf8proj schedule -l project.proj       # Enable resource leveling
utf8proj schedule -V project.proj       # Verbose: show [task_id] Display Name
utf8proj schedule --as-of 2025-03-15    # Progress-aware scheduling from status date

# Generate Gantt chart
utf8proj gantt project.proj -o timeline.svg           # SVG (default)
utf8proj gantt project.proj -o timeline.html -f html  # Interactive HTML
utf8proj gantt project.proj -o chart.xlsx -f xlsx     # Excel workbook
utf8proj gantt project.proj -o chart.xlsx -f xlsx --include-calendar --include-diagnostics

# Focus view (filter by task prefix)
utf8proj gantt project.proj -o impl.html -f html --focus="impl" --context-depth=1

# Fix MS Project import issues
utf8proj fix container-deps project.proj -o fixed.proj
```

## Library Usage

```rust
use utf8proj_core::{Project, Task, Resource, Duration, Scheduler};
use utf8proj_solver::CpmSolver;

// Build a project programmatically
let mut project = Project::new("My Project");
project.start = chrono::NaiveDate::from_ymd_opt(2025, 2, 1).unwrap();
project.resources = vec![Resource::new("dev").capacity(1.0)];
project.tasks = vec![
    Task::new("design").effort(Duration::days(5)),
    Task::new("implement")
        .effort(Duration::days(10))
        .depends_on("design")
        .assign("dev"),
];

// Schedule
let solver = CpmSolver::new();
let schedule = solver.schedule(&project)?;

println!("Project ends: {}", schedule.project_end);
println!("Critical path: {:?}", schedule.critical_path);

// With resource leveling
let solver = CpmSolver::with_leveling();
let schedule = solver.schedule(&project)?;
```

## Documentation

| Document | Purpose |
|----------|---------|
| [Quick Reference](QUICK_REFERENCE.md) | DSL syntax cheat sheet |
| [Grammar Specification](docs/GRAMMAR.md) | Complete `.proj` file syntax |
| [Diagnostics Reference](docs/DIAGNOSTICS.md) | All diagnostic codes and meanings |
| [Explainability Manifesto](docs/EXPLAINABILITY.md) | Core philosophy and design principles |
| [Design Philosophy](docs/DESIGN_PHILOSOPHY.md) | Architectural decisions |
| [Editor Setup](docs/EDITOR_SETUP.md) | VS Code, Neovim, Vim, Zed, Sublime Text |
| [Tutorial](docs/tutorial.md) | Getting started guide |

## Comparison

| Feature | utf8proj | TaskJuggler | MS Project |
|---------|----------|-------------|------------|
| **File Format** | Text (.proj) | Text (.tjp) | Binary (.mpp) |
| **Version Control** | Excellent | Good | Poor |
| **Explainability** | First-class | Limited | None |
| **Resource Leveling** | Deterministic | Optimizer | Black box |
| **Calendar Diagnostics** | 23 codes | None | None |
| **License** | MIT/Apache-2.0 | GPL-2.0 | Commercial |
| **Single Binary** | Yes | No (Ruby) | No |

## Architecture

```
┌─────────────────────────────────────────────┐
│                .proj Files                  │
│          (Plain Text, Git-Friendly)         │
└─────────────────────────────────────────────┘
                     │
┌─────────────────────────────────────────────┐
│           utf8proj-parser                   │
│      (pest grammar → Project model)         │
└─────────────────────────────────────────────┘
                     │
┌─────────────────────────────────────────────┐
│           utf8proj-solver                   │
│  (CPM scheduling, leveling, diagnostics)    │
└─────────────────────────────────────────────┘
                     │
┌─────────────────────────────────────────────┐
│         utf8proj-render + core              │
│   (Excel, SVG, Mermaid, CalendarImpact)     │
└─────────────────────────────────────────────┘
                     │
     ┌───────────────┼───────────────┐
     │               │               │
┌────▼─────┐   ┌────▼─────┐   ┌─────▼────┐
│   CLI    │   │   LSP    │   │  WASM    │
│          │   │ (Editor) │   │(Browser) │
└──────────┘   └──────────┘   └──────────┘
```

## Companion Tools

### MS Project Converter

Convert Microsoft Project (`.mpp`) files to utf8proj's native format:

```bash
cd tools/mpp_to_proj
./setup_companion.sh
source .venv/bin/activate
python mpp_to_proj.py project.mpp
```

See [tools/mpp_to_proj/README.md](tools/mpp_to_proj/README.md) for details.

## Development

```bash
git clone https://github.com/alanbld/utf8proj
cd utf8proj
cargo build --workspace
cargo test --workspace

# Run coverage
cargo tarpaulin --workspace --out Stdout
```

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.

## Acknowledgments

utf8proj is inspired by [TaskJuggler](https://taskjuggler.org/), a pioneering text-based project management tool. utf8proj is a clean-room implementation focused on **explainability** — making scheduling decisions transparent and auditable.

---

<div align="center">
  <sub>Built with Rust. Designed for transparency.</sub>
</div>
