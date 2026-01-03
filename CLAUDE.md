# utf8proj - Project Context

## Project Overview

Rust-based project scheduling engine with CPM (Critical Path Method) solver, resource leveling, and BDD-based conflict analysis. Parses TaskJuggler (.tjp) and native DSL (.proj) formats, schedules tasks, and renders output. Implements PMI/PMBOK scheduling standards.

## Workspace Structure

```
crates/
├── utf8proj-core/      # Core types: Task, Resource, Dependency, Calendar, Schedule
├── utf8proj-parser/    # Parsers for TJP and native DSL (pest grammar)
├── utf8proj-solver/    # CPM scheduler with resource leveling
│   ├── src/leveling.rs # Resource over-allocation detection and resolution
│   └── src/bdd.rs      # BDD-based conflict analysis (Biodivine)
├── utf8proj-render/    # Output rendering (multiple formats)
│   ├── src/gantt.rs    # Interactive HTML Gantt chart renderer
│   ├── src/mermaid.rs  # MermaidJS Gantt diagram
│   ├── src/plantuml.rs # PlantUML Gantt diagram
│   └── src/excel.rs    # Excel costing reports with dependencies
├── utf8proj-cli/       # Command-line interface (untested)
└── utf8proj-wasm/      # WebAssembly bindings for browser playground

playground/             # Browser-based playground
├── index.html          # Main HTML with Monaco editor
├── src/main.js         # JavaScript module (WASM integration)
├── styles/main.css     # Styling (light/dark themes)
├── build.sh            # Build script (wasm-pack)
└── pkg/                # WASM output (gitignored)
```

## Key Features Implemented

- **Hierarchical tasks**: Nested task parsing, container date derivation (min/max of children)
- **Dependency types**: FS (default), SS (!), FF (~), SF (!~) with lag support
- **Calendars**: Working days, working hours, holidays (single-date and range)
- **Resources**: Rate, capacity, efficiency, calendar, email, role, leave
- **Task attributes**: Priority, complete %, constraints, note, tag, cost, payment
- **Milestones**: Dedicated `milestone` declaration syntax
- **Constraints**: Declarative constraint blocks for what-if analysis
- **Critical path**: Calculation with all dependency types
- **Effort-driven scheduling**: PMI-compliant Duration = Effort / Resource_Units
- **Resource leveling**: Automatic over-allocation detection and task shifting
- **BDD conflict analysis**: Binary Decision Diagram-based conflict detection (Biodivine)
- **Interactive Gantt chart**: Standalone HTML output with SVG, tooltips, zoom, dependency arrows
- **Multiple render formats**: HTML, SVG, MermaidJS, PlantUML, Excel (XLSX)
- **Excel costing reports**: Formula-driven scheduling with dependency cascading
- **Browser playground**: WASM-based in-browser scheduler with Monaco editor

## Test Coverage (as of 2026-01-04)

| Module | Lines | Coverage |
|--------|-------|----------|
| utf8proj-parser/lib | 13/13 | 100.0% |
| utf8proj-render/excel | 421/432 | 97.5% |
| utf8proj-core | 127/131 | 96.9% |
| utf8proj-parser/native | 340/353 | 96.3% |
| utf8proj-solver/lib | 269/280 | 96.1% |
| utf8proj-solver/leveling | 187/196 | 95.4% |
| utf8proj-solver/bdd | 105/111 | 94.6% |
| utf8proj-solver/cpm | 78/83 | 94.0% |
| utf8proj-parser/tjp | 140/151 | 92.7% |
| utf8proj-render/gantt | 255/278 | 91.7% |
| utf8proj-render/plantuml | 109/119 | 91.6% |
| utf8proj-render/lib | 223/245 | 91.0% |
| utf8proj-solver/dag | 129/142 | 90.8% |
| utf8proj-render/mermaid | 96/111 | 86.5% |
| utf8proj-cli | 143/446 | 32.1% |
| utf8proj-wasm | 18/121 | 14.9% |
| **Overall** | **2710/3269** | **82.90%** |

**All core business logic components achieve 90%+ coverage** (excluding CLI and WASM).

**Tests:** 290+ passing, 1 ignored (render doctest)

**Test breakdown:**
- utf8proj-solver: 60 unit + 27 hierarchical + 8 correctness + 12 leveling = 107 tests
- utf8proj-render: 62 unit + 5 integration = 67 tests
- utf8proj-parser: 53 unit + 19 integration = 72 tests
- utf8proj-core: 14 tests + 1 doc-test
- utf8proj-cli: 10 tests
- utf8proj-wasm: 8 tests

## Effort-Driven Scheduling (PMI Compliant)

Duration is calculated from effort using the PMI formula:

```
Duration = Effort / Total_Resource_Units
```

### Examples

| Effort | Resources | Total Units | Duration |
|--------|-----------|-------------|----------|
| 40h | 1 @ 100% | 1.0 | 5 days |
| 40h | 1 @ 50% | 0.5 | 10 days |
| 40h | 2 @ 100% | 2.0 | 2.5 days |
| 40h | 1@100% + 1@50% | 1.5 | ~3.3 days |

### Usage

```rust
// Default 100% allocation
Task::new("work").effort(Duration::days(5)).assign("dev")

// Partial allocation (50%)
Task::new("work").effort(Duration::days(5)).assign_with_units("dev", 0.5)

// Fixed duration (ignores allocation)
Task::new("meeting").duration(Duration::days(1)).assign("dev")
```

## Resource Leveling

The solver now supports automatic resource leveling to resolve over-allocation conflicts.

### Usage

```rust
use utf8proj_solver::CpmSolver;
use utf8proj_core::Scheduler;

// Without leveling (default)
let solver = CpmSolver::new();

// With resource leveling enabled
let solver = CpmSolver::with_leveling();
let schedule = solver.schedule(&project).unwrap();
```

### API

```rust
use utf8proj_solver::{detect_overallocations, level_resources, LevelingResult};

// Detect over-allocations without resolving
let conflicts = detect_overallocations(&project, &schedule);

// Manually level resources with full result
let result: LevelingResult = level_resources(&project, &schedule, &calendar);
// result.shifted_tasks - tasks that were moved
// result.unresolved_conflicts - conflicts that couldn't be resolved
// result.project_extended - whether project duration increased
```

### Algorithm

1. Build resource usage timeline for each resource
2. Detect over-allocation periods (usage > capacity)
3. For each conflict:
   - Find candidate tasks to shift (prioritize non-critical, higher slack, lower priority)
   - Shift task to next available slot
4. Repeat until resolved or impossible
5. Recalculate critical path if project was extended

## Interactive Gantt Chart

Generate standalone HTML files with interactive SVG Gantt charts.

### Usage

```rust
use utf8proj_render::HtmlGanttRenderer;
use utf8proj_core::Renderer;

// Basic usage
let renderer = HtmlGanttRenderer::new();
let html = renderer.render(&project, &schedule)?;

// With dark theme
let renderer = HtmlGanttRenderer::new().dark_theme();

// Static (no JS interactivity)
let renderer = HtmlGanttRenderer::new().static_chart();

// Hide dependency arrows
let renderer = HtmlGanttRenderer::new().hide_dependencies();
```

### Features

- **Task bars**: Color-coded by critical path status
- **Dependency arrows**: FS, SS, FF, SF with curved paths
- **Tooltips**: Hover for task details (name, dates, duration)
- **Zoom controls**: +/- buttons and reset
- **Hierarchical tasks**: Container brackets, indentation
- **Milestones**: Diamond shapes
- **Themes**: Light (default) and dark

## MermaidJS Renderer

Generate Mermaid Gantt diagrams for Markdown documentation.

```rust
use utf8proj_render::MermaidRenderer;
use utf8proj_core::Renderer;

let renderer = MermaidRenderer::new();
let mermaid = renderer.render(&project, &schedule)?;
// Output: gantt\n  title Project\n  section Tasks\n  Design: crit, 2025-01-06, 5d\n...
```

### Features

- Critical path markers (`crit`)
- Milestone detection
- Dependency syntax (`after taskId`)
- Weekend exclusion
- Section grouping

## PlantUML Renderer

Generate PlantUML Gantt diagrams for wikis and documentation.

```rust
use utf8proj_render::PlantUmlRenderer;
use utf8proj_core::Renderer;

let renderer = PlantUmlRenderer::new();
let plantuml = renderer.render(&project, &schedule)?;
// Output: @startgantt\n[Design] starts 2025-01-06 and lasts 5 days\n...
```

### Features

- Critical path coloring (configurable colors)
- Dependency syntax (`starts at [X]'s end`)
- Milestone markers (`happens at`)
- Weekend closure
- Scale options (day/week/month)
- Today marker

## Excel Costing Report Renderer

Generate XLSX files with formula-driven scheduling and dependency cascading.

```rust
use utf8proj_render::ExcelRenderer;
use utf8proj_core::Renderer;

let renderer = ExcelRenderer::new()
    .currency("€")
    .weeks(24)
    .hours_per_day(8.0);

let xlsx_bytes = renderer.render(&project, &schedule)?;
std::fs::write("project_cost.xlsx", xlsx_bytes)?;
```

### Sheets Generated

1. **Profiles and Costs**: Resource rates, effort totals, cost calculations
2. **Schedule**: Week-based Gantt with formula-driven hours distribution
3. **Executive Summary**: Project overview with total effort and cost

### Dependency Support (Live Scheduling)

When dependencies are enabled (default), the Schedule sheet includes:

| Column | Purpose |
|--------|---------|
| Task ID | Unique ID for VLOOKUP |
| Depends On | Predecessor task ID |
| Type | FS/SS/FF/SF |
| Lag (d) | Lead/lag time |
| Start Week | **Formula**: `=VLOOKUP(predecessor_end) + 1 + lag` |
| End Week | **Formula**: `=Start + CEILING(effort/week_hours)` |

**Cascade Effect**: Change a task's effort → End recalculates → Successor Start recalculates → All dependent tasks shift automatically.

### Configuration

```rust
// Default: dependencies enabled, formulas enabled
let renderer = ExcelRenderer::new();

// Disable dependencies (simpler output)
let renderer = ExcelRenderer::new().no_dependencies();

// Static values instead of formulas
let renderer = ExcelRenderer::new().static_values();

// Custom work week
let renderer = ExcelRenderer::new()
    .hours_per_day(8.0)
    .hours_per_week(35.0);  // Part-time
```

## Recent Work Completed

1. **PMI-Compliant Effort Scheduling** (`crates/utf8proj-solver/src/lib.rs`)
   - Fixed effort-to-duration calculation: `Duration = Effort / Total_Resource_Units`
   - Added `Task::assign_with_units()` for partial allocations
   - 7 new tests for effort-driven scheduling scenarios
   - See `docs/SCHEDULING_ANALYSIS.md` for full PMI compliance review

2. **BDD Conflict Analysis** (`crates/utf8proj-solver/src/bdd.rs`)
   - `BddConflictAnalyzer` using Biodivine library
   - Encodes resource conflicts as Boolean satisfiability
   - Finds optimal resolution via BDD traversal
   - 5 tests for BDD functionality

3. **CLI Enhancements** (`crates/utf8proj-cli/src/main.rs`)
   - Added `-l/--leveling` flag to `schedule` command
   - Added `bdd-benchmark` subcommand for BDD vs heuristic comparison
   - BDD benchmark scenarios: SingleResource, MultiResource, ResourceWeb

4. **WASM Playground** (`crates/utf8proj-wasm/`, `playground/`)
   - `Playground` struct with WASM bindings for schedule/render/validate
   - Monaco editor with custom syntax highlighting for TJP and native DSL
   - Real-time validation with error markers
   - Live Gantt chart preview (HTML/SVG)
   - Share functionality (URL-encoded projects)
   - Light/dark theme toggle
   - Example templates for both formats

5. **Interactive Gantt Chart** (`crates/utf8proj-render/src/gantt.rs`)
   - `HtmlGanttRenderer` - Generates standalone HTML with embedded SVG
   - Dependency arrows with curved paths
   - Tooltips and zoom controls
   - Light and dark themes

6. **Resource Leveling** (`crates/utf8proj-solver/src/leveling.rs`)
   - `ResourceTimeline` - tracks resource usage by day
   - `detect_overallocations` - finds over-allocation periods
   - `level_resources` - resolves conflicts by shifting tasks
   - 12 integration tests covering various scenarios

7. **MermaidJS Renderer** (`crates/utf8proj-render/src/mermaid.rs`)
   - `MermaidRenderer` - Generates Mermaid Gantt syntax
   - Critical path markers, milestone detection, dependency syntax
   - Weekend exclusion, section grouping
   - 12 tests

8. **PlantUML Renderer** (`crates/utf8proj-render/src/plantuml.rs`)
   - `PlantUmlRenderer` - Generates PlantUML Gantt syntax
   - Critical path coloring, dependency syntax, milestone markers
   - Weekend closure, scale options, today marker
   - 17 tests

9. **Excel Costing Report** (`crates/utf8proj-render/src/excel.rs`)
   - `ExcelRenderer` - Generates XLSX files using rust_xlsxwriter
   - Multiple sheets: Profiles/Costs, Schedule (Gantt), Executive Summary
   - Formula-driven scheduling with VLOOKUP for dependencies
   - All dependency types (FS/SS/FF/SF) with lag support
   - Cascade effect: change effort → all successors recalculate
   - 13 tests

10. **Extended Native DSL Grammar** (`crates/utf8proj-parser/src/native/grammar.pest`)
    - Project: `timezone:` attribute
    - Resources: `email:`, `role:`, `leave:` attributes
    - Tasks: `note:`, `tag:`, `cost:`, `payment:` attributes
    - Milestones: Dedicated `milestone id "name" { }` declaration
    - Reports: Extended with `title:`, `type:`, `show:`, `scale:`, `width:`, `breakdown:`, `period:`
    - Constraints: Declarative `constraint id { }` blocks for what-if analysis
    - Holidays: Single-date support (not just ranges)
    - Resource refs: Both `@50%` and `(50%)` syntax for partial allocation

11. **Tutorial & Benchmark Documentation** (`docs/`)
    - `tutorial.md` - Step-by-step guide using CRM migration example
    - `benchmark-report.md` - TaskJuggler comparison and adoption readiness
    - Full comparison of syntax, features, and performance

## Important Files

- `crates/utf8proj-solver/src/lib.rs` - CPM scheduler with effort-driven calculation
- `crates/utf8proj-solver/src/leveling.rs` - Resource leveling algorithm
- `crates/utf8proj-solver/src/bdd.rs` - BDD-based conflict analysis
- `crates/utf8proj-render/src/gantt.rs` - Interactive HTML Gantt chart renderer
- `crates/utf8proj-render/src/mermaid.rs` - MermaidJS Gantt renderer
- `crates/utf8proj-render/src/plantuml.rs` - PlantUML Gantt renderer
- `crates/utf8proj-render/src/excel.rs` - Excel costing report with dependencies
- `crates/utf8proj-parser/src/native/mod.rs` - Native DSL parser
- `crates/utf8proj-parser/src/native/grammar.pest` - Native DSL grammar
- `crates/utf8proj-parser/src/tjp/mod.rs` - TaskJuggler parser
- `crates/utf8proj-core/src/lib.rs` - Core types and traits
- `docs/SCHEDULING_ANALYSIS.md` - PMI/PERT/CPM compliance analysis
- `docs/tutorial.md` - Step-by-step tutorial (CRM migration example)
- `docs/benchmark-report.md` - TaskJuggler comparison and adoption readiness

## Example Projects

```
examples/
├── crm_migration.proj   # Full-featured CRM project (native DSL)
├── crm_migration.tjp    # TaskJuggler equivalent
├── crm_simple.proj      # Simplified version for testing
└── crm_simple.tjp       # Simplified TJP version
```

The CRM Migration example demonstrates:
- 28 tasks across 5 phases (Discovery, Data Migration, Integration, Deployment, Hypercare)
- 6 resources with varying rates and capacities
- Parallel tracks with convergence points
- Milestones with payment triggers
- All dependency types with lag

## Related Project

TJP example files are in sibling directory:
`/home/albalda/projects/msproject-to-taskjuggler/examples/ttg_*.tjp`

## Commands

```bash
# Run all tests
cargo test --workspace

# Check coverage
cargo tarpaulin --workspace --out Stdout --skip-clean

# Build release
cargo build --release

# Schedule a project
target/release/utf8proj schedule project.tjp

# Schedule with resource leveling
target/release/utf8proj schedule -l project.tjp

# Generate Gantt chart
target/release/utf8proj gantt project.tjp -o gantt.svg

# Run benchmarks
target/release/utf8proj benchmark -t chain -c 10000 --series
target/release/utf8proj bdd-benchmark --series

# Build WASM and run playground
cd playground && ./build.sh
python3 -m http.server 8080
# Open http://localhost:8080
```

## Remaining Work

- CLI test coverage (0% currently)
- Edge cases in calendar parsing (lines 312-316, 326, 329)
- Some resource/task attribute combinations in native parser
- Error handling paths in leveling

## Grammar Notes

### Native DSL (.proj)

**Project attributes:**
- `start:`, `end:`, `currency:`, `calendar:`, `timezone:`

**Resource attributes:**
- `rate: 850/day` or `rate: 100/hour`
- `capacity: 0.75` (75% allocation)
- `efficiency: 1.2` (productivity factor)
- `email: "user@company.it"`
- `role: "Solution Architect"`
- `leave: 2026-03-02..2026-03-13`

**Task attributes:**
- `effort: 15d` (person-time, divided among assignees)
- `duration: 2w` (fixed calendar time)
- `assign: sa1, sa2` or `assign: dev1@50%` or `assign: dev1(50%)`
- `depends: task`, `depends: phase.task`, `depends: a, b`
- `priority: 800` (higher = scheduled first)
- `note: "Description text"`
- `tag: critical, integration`
- `cost: 500` (fixed cost)
- `payment: 25000` (milestone payment)
- `milestone: true` or dedicated `milestone id "name" { }` syntax
- `complete: 75%` (progress tracking)

**Dependency syntax:**
- `depends: a` (FS - Finish-to-Start, default)
- `depends: a SS` (Start-to-Start)
- `depends: a FF` (Finish-to-Finish)
- `depends: a SF` (Start-to-Finish)
- `depends: a +2d` (lag: start 2 days after)
- `depends: a -1d` (lead: start 1 day before)

**Holidays:**
- Single date: `holiday "Easter" 2026-04-06`
- Date range: `holiday "Christmas" 2025-12-25..2025-12-26`

**Constraints (declarative blocks):**
```proj
constraint hard_deadline {
    type: soft
    target: deployment.golive_complete
    condition: end <= 2026-05-01
    priority: 900
}
```

**Reports:**
```proj
report gantt "output/timeline.svg" {
    title: "Project Schedule"
    tasks: all
    show: resources, critical_path
    scale: week
    width: 1200
}
```

### TaskJuggler (.tjp)

- `!` prefix for sibling references: `depends !kickoff`
- `~` suffix for FF: `depends task~`
- `!~` for SF: `depends !task~`
- Resource allocation: `allocate dev1, dev2`
- Leaves: `leaves annual 2026-03-02 - 2026-03-13`
