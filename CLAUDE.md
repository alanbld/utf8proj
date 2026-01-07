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
├── utf8proj-cli/       # Command-line interface
├── utf8proj-lsp/       # Language Server Protocol implementation
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

## Test Coverage (as of 2026-01-05)

| Module | Lines | Coverage |
|--------|-------|----------|
| utf8proj-lsp/hover | 120/120 | 100.0% |
| utf8proj-render/mermaid | 111/111 | 100.0% |
| utf8proj-parser/lib | 13/13 | 100.0% |
| utf8proj-render/lib | 243/245 | 99.2% |
| utf8proj-solver/lib | 592/598 | 99.0% |
| utf8proj-render/gantt | 273/278 | 98.2% |
| utf8proj-render/excel | 421/432 | 97.5% |
| utf8proj-solver/bdd | 108/111 | 97.3% |
| utf8proj-solver/leveling | 192/196 | 98.0% |
| utf8proj-parser/tjp | 152/157 | 96.8% |
| utf8proj-parser/native | 455/471 | 96.6% |
| utf8proj-render/plantuml | 115/119 | 96.6% |
| utf8proj-solver/dag | 136/142 | 95.8% |
| utf8proj-solver/cpm | 78/83 | 94.0% |
| utf8proj-core | 408/444 | 91.9% |
| utf8proj-wasm | 74/81 | 91.4% |
| utf8proj-lsp/diagnostics | 28/31 | 90.3% |
| utf8proj-cli | 271/638 | 42.5% |
| **Overall** | **3857/4464** | **86.40%** |

**All core business logic components achieve 90%+ coverage** (excluding CLI entry point).

**Tests:** 606 passing, 1 ignored (render doctest)

**Test breakdown:**
- utf8proj-solver: 102 unit + 27 hierarchical + 13 correctness + 12 leveling + 4 progress + 19 semantic = 177 tests
- utf8proj-render: 80 unit + 25 integration = 105 tests
- utf8proj-parser: 79 unit + 19 integration = 98 tests
- utf8proj-core: 74 tests + 5 doc-tests = 79 tests
- utf8proj-cli: 32 unit + 14 diagnostic snapshot + 26 exit code = 72 tests
- utf8proj-lsp: 5 diagnostic + 39 hover = 44 tests
- utf8proj-wasm: 15 tests

## Diagnostic System (Compiler-Grade)

The CLI implements rustc-style diagnostics for project analysis with structured output and CI-ready exit codes.

### Exit Code Contract (Stable API)

| Exit Code | Meaning |
|-----------|---------|
| 0 | Success: no errors (warnings/hints/info allowed) |
| 1 | Failure: one or more errors emitted |

### Policy Flags

- **`--strict`**: Escalates severities (warnings→errors, hints→warnings)
- **`--quiet`**: Suppresses all output except errors (does NOT change exit code)
- **`--format=json`**: Machine-readable output (same exit semantics)

### Diagnostic Codes

| Code | Severity | Trigger |
|------|----------|---------|
| E001 | Error | Circular specialization in profiles |
| E002 | Warning | Profile without rate assigned to tasks |
| E003 | Error | Infeasible constraint (cannot be satisfied) |
| W001 | Warning | Task assigned to abstract profile |
| W002 | Warning | Wide cost range (>100% spread) |
| W003 | Warning | Unknown trait on profile |
| W004 | Warning | Approximate leveling applied |
| H001 | Hint | Mixed abstract and concrete assignments |
| H002 | Hint | Unused profile defined |
| H003 | Hint | Unused trait defined |
| I001 | Info | Project scheduled successfully (summary) |
| I003 | Info | Resource utilization report |
| I004 | Info | Project status (progress + variance) |
| I005 | Info | Earned value summary (SPI) |

### Usage

```bash
# Fast validation (no schedule output) - ideal for CI/pre-commit
utf8proj check project.proj             # Exit 0 with warnings
utf8proj check --strict project.proj    # Exit 1 with warnings
utf8proj check --quiet --strict project.proj  # Silent, exit code only
utf8proj check --format=json project.proj     # Machine-readable

# Full scheduling with output
utf8proj schedule project.proj          # Exit 0 with warnings
utf8proj schedule --strict project.proj # Exit 1 with warnings
utf8proj schedule --quiet --strict project.proj
utf8proj schedule --format=json project.proj
utf8proj schedule --task-ids project.proj  # Show task IDs instead of display names
utf8proj schedule -V project.proj         # Verbose: show [task_id] Display Name
utf8proj schedule -w 60 project.proj      # Custom task column width (default: 40)
```

The `check` command is analogous to `cargo check`, `terraform validate`, or `tsc --noEmit` - it runs parse + semantic analysis without producing schedule output.

### Implementation Files

- `crates/utf8proj-cli/src/diagnostics.rs` - Emitters, ExitCode, DiagnosticConfig
- `crates/utf8proj-core/src/diagnostics.rs` - Core types (Diagnostic, Severity, DiagnosticCode)
- `crates/utf8proj-solver/src/lib.rs` - analyze_project() emission points
- `crates/utf8proj-cli/tests/exit_codes.rs` - 19 integration tests
- `crates/utf8proj-cli/tests/diagnostics.rs` - 14 snapshot tests

## Language Server Protocol (LSP)

The `utf8proj-lsp` crate provides IDE support for `.proj` files via the Language Server Protocol.

### Server Capabilities

| Capability | Description |
|------------|-------------|
| `textDocumentSync` | Full document sync on open/change |
| `hoverProvider` | Contextual info for identifiers |
| `documentSymbolProvider` | Outline of profiles, resources, tasks |

### Features

- **Real-time diagnostics**: Parse errors and semantic warnings as you type
- **Hover information**:
  - Profiles: rate range, specialization chain, traits, skills
  - Resources: rate, capacity, efficiency
  - Tasks: duration, effort, assignments, dependencies
  - Traits: description, rate multiplier
- **Document symbols**: Navigate profiles, resources, and tasks

### Usage

```bash
# Build the LSP server
cargo build --release -p utf8proj-lsp

# Run (connects via stdio)
./target/release/utf8proj-lsp
```

### Editor Integration

**VS Code** (with generic LSP extension):
```json
{
  "languageServerExample.serverPath": "./target/release/utf8proj-lsp",
  "languageServerExample.fileExtensions": [".proj"]
}
```

**Neovim** (with nvim-lspconfig):
```lua
require('lspconfig.configs').utf8proj = {
  default_config = {
    cmd = { './target/release/utf8proj-lsp' },
    filetypes = { 'proj' },
    root_dir = function(fname)
      return vim.fn.getcwd()
    end,
  },
}
require('lspconfig').utf8proj.setup{}
```

### Implementation Files

- `crates/utf8proj-lsp/src/main.rs` - tower-lsp server, Backend impl
- `crates/utf8proj-lsp/src/diagnostics.rs` - Diagnostic → LSP conversion
- `crates/utf8proj-lsp/src/hover.rs` - Hover info for profiles/resources/tasks

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

1. **RFC-0001: Progressive Resource Refinement Grammar** (`crates/utf8proj-parser/src/native/grammar.pest`)
   - Added `resource_profile` declaration with specializes, skills, traits, rate ranges
   - Added `trait` declaration with description and rate_multiplier
   - Added rate range block syntax (min/max/currency) for profiles and resources
   - Added quantified assignment syntax: `assign: developer * 2`
   - Extended resource_ref to support both percentage (@50%) and quantity (*N)
   - Full RFC in `docs/rfc/RFC-0001-PROGRESSIVE-RESOURCE-REFINEMENT.md`

2. **PMI-Compliant Effort Scheduling** (`crates/utf8proj-solver/src/lib.rs`)
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

# Validate a project (fast, no output)
target/release/utf8proj check project.proj
target/release/utf8proj check --strict project.proj  # warnings→errors
target/release/utf8proj check --format=json project.proj

# Schedule a project
target/release/utf8proj schedule project.tjp

# Schedule with resource leveling
target/release/utf8proj schedule -l project.tjp

# Generate Gantt chart
target/release/utf8proj gantt project.tjp -o gantt.svg              # SVG (default)
target/release/utf8proj gantt project.tjp -o chart.mmd -f mermaid   # MermaidJS
target/release/utf8proj gantt project.tjp -o chart.puml -f plantuml # PlantUML
target/release/utf8proj gantt project.tjp -o chart.xlsx -f xlsx     # Excel workbook
target/release/utf8proj gantt project.tjp -o chart.svg -V           # Verbose: [id] Name
target/release/utf8proj gantt project.tjp -o chart.svg --task-ids   # Task IDs only
target/release/utf8proj gantt project.tjp -o chart.svg -w 60        # Custom label width

# Excel-specific options
target/release/utf8proj gantt project.tjp -o chart.xlsx -f xlsx --currency EUR --weeks 40

# Run benchmarks
target/release/utf8proj benchmark -t chain -c 10000 --series
target/release/utf8proj bdd-benchmark --series

# Build WASM and run playground
cd playground && ./build.sh
python3 -m http.server 8080
# Open http://localhost:8080
```

## Bugs Identified and Fixed (2026-01-04)

### TJP Parser: Choice Rule Unwrapping
**Location:** `crates/utf8proj-parser/src/tjp/mod.rs`

**Problem:** The `project_attr` and `resource_attr` choice rules in the pest grammar were not being unwrapped before matching on the actual attribute type. This caused attributes like `currency` and `efficiency` to be silently ignored.

**Root Cause:** Pest returns choice rules as their container type (e.g., `project_attr`) rather than the matched alternative (e.g., `currency_attr`). The code was matching on `Rule::currency_attr` but the actual rule was `Rule::project_attr`.

**Fix:** Added unwrapping logic to extract the actual attribute from the choice rule before matching:
```rust
let actual_attr = if attr.as_rule() == Rule::project_attr {
    attr.into_inner().next().unwrap()
} else {
    attr
};
```

**Tests Added:** `parse_project_currency`, `parse_resource_efficiency`, `parse_dependency_onstart`, `parse_dependency_onend`

### Fixed: CPM Backward Pass for Non-FS Dependencies (2026-01-05)
**Location:** `crates/utf8proj-solver/src/lib.rs` (lines 1331-1389)

**Problem:** The backward pass was treating all dependencies as FS, computing `LF(pred) = min(LS(succ))` regardless of dependency type. This caused incorrect slack calculations for SS/FF/SF dependencies.

**Root Cause:** The `successors_map` only stored successor IDs without dependency type information, and the backward pass code used a simplified formula.

**Fix:** Enhanced backward pass to look up the actual dependency type from the successor task's `depends` list and apply the correct formula:
- **FS:** `LF(pred) <= LS(succ) - lag`
- **SS:** `LF(pred) <= LS(succ) - lag + duration(pred)`
- **FF:** `LF(pred) <= LF(succ) - lag`
- **SF:** `LF(pred) <= LF(succ) - lag + duration(pred)`

**Tests Added:** 5 new tests in `crates/utf8proj-solver/tests/cpm_correctness.rs`:
- `ff_forward_pass_accounts_for_successor_duration`
- `ss_backward_pass_accounts_for_predecessor_duration`
- `ss_with_lag_forward_pass`
- `ff_with_lag_forward_pass`
- `mixed_dependency_types_correct_critical_path`

**Impact:** Correct slack calculations for all dependency types, enabling accurate critical path analysis for projects using SS/FF/SF dependencies.

## Remaining Work

- CLI test coverage (32.1% currently)
- WASM test coverage (14.9% currently)
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

**Task naming:**
```proj
task impl_api "Implement Backend API" {   # Quoted string = display name
    summary: "Backend API"                 # Optional short name (supplementary)
    duration: 5d
}
```
- `name` (quoted string): Primary display name, human-readable description
- `summary:`: Optional short display name (supplementary info)
- Display fallback: `name` → `id`

**Task attributes:**
- `effort: 15d` (person-time, divided among assignees)
- `duration: 2w` (fixed calendar time)
- `assign: sa1, sa2` or `assign: dev1@50%` or `assign: dev1(50%)`
- `depends: task`, `depends: phase.task`, `depends: a, b`
- `priority: 800` (higher = scheduled first)
- `summary: "Short Name"` (optional display name)
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
- Holidays: `leaves holiday "Name" 2026-01-01` or `vacation 2026-01-01`
- Task naming: quoted string → `summary`, `note` → supplementary info

**String escape sequences (native DSL):**
- `\"` - escaped quote
- `\\` - escaped backslash
- `\n` - newline
- `\t` - tab
