# utf8proj - Project Context

## Project Overview

Rust-based **explainable** project scheduling engine with CPM (Critical Path Method) solver and deterministic resource leveling. Parses TaskJuggler (.tjp) and native DSL (.proj) formats, schedules tasks, and renders output. Core philosophy: **"describe, don't prescribe"** — diagnostics explain scheduling decisions rather than silently fixing them.

## Workspace Structure

```
crates/
├── utf8proj-core/      # Core types: Task, Resource, Dependency, Calendar, Schedule
├── utf8proj-parser/    # Parsers for TJP and native DSL (pest grammar)
├── utf8proj-solver/    # CPM scheduler with deterministic resource leveling
│   ├── src/leveling.rs # RFC-0003 resource leveling with audit trail
│   └── src/bdd.rs      # BDD-based conflict analysis (experimental)
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

syntax/                 # Editor syntax highlighting
├── utf8proj.tmLanguage.json  # TextMate grammar (VS Code, Sublime, Zed)
├── proj.vim                   # Vim syntax (Neovim, Vim)
└── ftdetect/
    └── proj.vim              # Filetype detection for Vim

tools/
└── mpp_to_proj/        # MS Project companion tool (Python)
    ├── mpp_to_proj.py       # Converts .mpp → .proj/.xml
    └── test_mpp_to_proj.py  # Unit tests (99% coverage)

.github/
└── workflows/
    └── release.yml     # Cross-platform release workflow (tag-triggered)
```

## Key Features Implemented

- **Hierarchical tasks**: Nested task parsing, container date derivation (min/max of children)
- **Dependency types**: FS (default), SS, FF, SF with lag support (+2d, -1d)
- **Calendars**: Working days, working hours, holidays (single-date and range)
- **Resources**: Rate, capacity, efficiency, calendar, email, role, leave
- **Task attributes**: Priority, complete %, constraints, note, tag, cost, payment
- **Milestones**: Dedicated `milestone` declaration syntax
- **Constraints**: Declarative constraint blocks for what-if analysis
- **Critical path**: Calculation with all dependency types
- **Effort-driven scheduling**: PMI-compliant Duration = Effort / Resource_Units
- **Resource leveling**: RFC-0003 deterministic leveling with full audit trail (L001-L004 diagnostics)
- **Progress-aware scheduling**: RFC-0008 status date resolution, remaining duration calculation, P005/P006 diagnostics
- **Calendar diagnostics**: C001-C023 codes for working days vs calendar days analysis
- **BDD conflict analysis**: Binary Decision Diagram-based conflict detection (experimental)
- **Interactive Gantt chart**: Standalone HTML output with SVG, tooltips, zoom, dependency arrows
- **Multiple render formats**: HTML, SVG, MermaidJS, PlantUML, Excel (XLSX)
- **Excel costing reports**: Formula-driven scheduling with dependency cascading
- **Browser playground**: WASM-based in-browser scheduler with Monaco editor

## Test Coverage (as of 2026-01-10)

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
| utf8proj-parser/tjp | 152/157 | 96.8% |
| utf8proj-parser/native | 455/471 | 96.6% |
| utf8proj-render/plantuml | 115/119 | 96.6% |
| utf8proj-solver/dag | 136/142 | 95.8% |
| utf8proj-solver/cpm | 78/83 | 94.0% |
| utf8proj-solver/leveling | 313/336 | 93.2% |
| utf8proj-core | 408/444 | 91.9% |
| utf8proj-wasm | 74/81 | 91.4% |
| utf8proj-lsp/diagnostics | 28/31 | 90.3% |
| utf8proj-cli | 271/638 | 42.5% |
| **Overall** | **~3900/4500** | **~86%** |

**All core business logic components achieve 90%+ coverage** (excluding CLI entry point).

**Tests:** 769 passing, 1 ignored (render doctest)

**Test breakdown:**
- utf8proj-solver: 102 unit + 27 hierarchical + 13 correctness + 25 leveling + 12 progress-aware + 4 variance + 19 semantic = 202 tests
- utf8proj-render: 80 unit + 25 integration = 105 tests
- utf8proj-parser: 79 unit + 19 integration = 98 tests
- utf8proj-core: 74 tests + 5 doc-tests = 79 tests
- utf8proj-cli: 32 unit + 14 diagnostic snapshot + 26 exit code + 3 fix command = 75 tests
- utf8proj-lsp: 5 diagnostic + 49 hover + 7 navigation = 61 tests
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
| **Leveling (L)** | | |
| L001 | Info | Overallocation resolved (task shifted) |
| L002 | Warning | Unresolvable conflict (no valid shift) |
| L003 | Info | Project duration increased due to leveling |
| L004 | Warning | Milestone delayed by leveling |
| **Progress (P)** | | |
| P005 | Warning | Remaining duration conflicts with complete% (e.g., remaining > 0 but 100% complete) |
| P006 | Warning | Container explicit complete% differs from derived by >10% |
| **Calendar (C)** | | |
| C001-C023 | Various | Calendar impact diagnostics (working days vs calendar days, weekend impact, holiday impact, etc.) |

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
| `definitionProvider` | Go-to-definition for symbols |
| `referencesProvider` | Find all references to symbols |

### Features

- **Real-time diagnostics**: Parse errors and semantic warnings as you type
- **Hover information**:
  - Profiles: rate range, specialization chain, traits, skills
  - Resources: rate, capacity, efficiency
  - Tasks: duration, effort, assignments, dependencies, calendar impact
  - Traits: description, rate multiplier
  - Diagnostics: L001-L004 (leveling), C001-C023 (calendar)
- **Document symbols**: Navigate profiles, resources, and tasks
- **Go-to-definition**: Jump to task, resource, calendar, profile, trait declarations
- **Find references**: Find all usages of a symbol (depends, assign, calendar, etc.)

### Usage

```bash
# Build the LSP server
cargo build --release -p utf8proj-lsp

# Run (connects via stdio)
./target/release/utf8proj-lsp
```

### Editor Integration

See `docs/EDITOR_SETUP.md` for detailed setup instructions.

| Editor | Syntax Highlighting | LSP Support |
|--------|---------------------|-------------|
| VS Code | TextMate grammar | Generic LSP extension |
| Neovim | Vim syntax file | Native (nvim-lspconfig) |
| Traditional Vim | Vim syntax file | None (requires plugins) |
| Zed | TextMate grammar | Planned |
| Sublime Text | TextMate grammar | Generic LSP plugin |

**Neovim** (recommended for full experience):
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

> **Note:** Traditional Vim has no built-in LSP support. Use Neovim for diagnostics, hover, and navigation features.

### Implementation Files

- `crates/utf8proj-lsp/src/main.rs` - tower-lsp server, Backend impl
- `crates/utf8proj-lsp/src/diagnostics.rs` - Diagnostic → LSP conversion
- `crates/utf8proj-lsp/src/hover.rs` - Hover info for profiles/resources/tasks
- `crates/utf8proj-lsp/src/navigation.rs` - Go-to-definition, find-references

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

## Resource Leveling (RFC-0003)

Deterministic, explainable resource leveling per RFC-0003. Key principles:
- **Opt-in only**: Leveling never runs unless explicitly requested
- **Deterministic**: Same inputs always produce identical outputs (stable sorting)
- **Auditable**: Every shift has a `LevelingReason` explaining why

### Usage

```rust
use utf8proj_solver::{CpmSolver, level_resources_with_options, LevelingOptions, LevelingStrategy};
use utf8proj_core::Scheduler;

// Without leveling (default)
let solver = CpmSolver::new();

// With resource leveling enabled
let solver = CpmSolver::with_leveling();
let schedule = solver.schedule(&project).unwrap();

// Manual leveling with options
let options = LevelingOptions {
    strategy: LevelingStrategy::CriticalPathFirst,
    max_project_delay_factor: Some(1.5), // Max 50% project extension
};
let result = level_resources_with_options(&project, &schedule, &calendar, &options);
```

### API

```rust
use utf8proj_solver::{detect_overallocations, level_resources_with_options, LevelingResult, LevelingReason, LevelingMetrics};

// Detect over-allocations without resolving
let conflicts = detect_overallocations(&project, &schedule);

// Full leveling with options
let result: LevelingResult = level_resources_with_options(&project, &schedule, &calendar, &options);
// result.original_schedule - unchanged input schedule (audit trail)
// result.leveled_schedule - schedule after leveling
// result.shifted_tasks - Vec<(task_id, original_start, new_start, LevelingReason)>
// result.unresolved_conflicts - conflicts that couldn't be resolved
// result.metrics - LevelingMetrics (duration increase, utilization, delays)
// result.diagnostics - L001-L004 diagnostics emitted
```

### LevelingReason Enum

```rust
pub enum LevelingReason {
    ResourceOverallocated {
        resource: String,
        peak_demand: f32,
        capacity: f32,
        dates: (NaiveDate, NaiveDate),
    },
    DependencyChain {
        predecessor: String,
        predecessor_delay: i64,
    },
}
```

### Algorithm (CriticalPathFirst Strategy)

1. Build resource usage timeline for each resource
2. Detect over-allocation periods (usage > capacity)
3. Sort conflicts deterministically by (resource_id, start_date)
4. For each conflict:
   - Find candidate tasks to shift
   - Sort candidates by: (is_critical ASC, slack DESC, priority DESC, task_id ASC)
   - Shift first viable candidate to next available slot
5. Emit L001 diagnostic for each resolution
6. Emit L002/L003/L004 as appropriate
7. Return both original and leveled schedules for comparison

## Progress-Aware Scheduling (RFC-0008)

Progress-aware CPM scheduling that respects task completion status when forecasting remaining work.

### Key Concepts

- **Status Date**: The "as-of" date for progress reporting. Tasks are classified relative to this date.
- **Task States**: Complete (100%), InProgress (0-99%), NotStarted (future tasks)
- **Remaining Duration**: Calculated from `duration * (1 - complete%)` or explicit `remaining:` override

### Status Date Resolution Chain

```
CLI --as-of flag > project.status_date > today()
```

### Task State Classification

| State | Condition | Forward Pass Behavior |
|-------|-----------|----------------------|
| Complete | `complete = 100%` | Locks to actual dates, no rescheduling |
| InProgress | `0 < complete < 100%` | Schedules remaining work from status date |
| NotStarted | `complete = 0%` | Schedules from predecessor completion |

### Usage

```proj
project ttg "TTG Migration" {
    start: 2026-01-06
    status_date: 2026-01-20  # Progress reporting date
}

task design "Design Phase" {
    duration: 10d
    complete: 100%           # Fully complete, locked
}

task develop "Development" {
    duration: 20d
    complete: 40%            # 40% done, 12d remaining
    remaining: 8d            # Override: explicit remaining (wins over calculated)
}
```

### Diagnostics

| Code | Trigger |
|------|---------|
| P005 | `remaining > 0` but `complete = 100%`, or `remaining = 0` but `complete < 100%` |
| P006 | Container's explicit `complete%` differs from weighted child rollup by >10% |

### Implementation Files

- `crates/utf8proj-solver/src/lib.rs` - `ProgressState` enum, `classify_progress_state()`, progress-aware forward pass
- `crates/utf8proj-core/src/lib.rs` - `Task.explicit_remaining` field
- `crates/utf8proj-solver/tests/progress_aware_cpm.rs` - 12 tests
- `docs/rfc/RFC-0008-PROGRESS-AWARE-CPM.md` - Full RFC specification

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

1. **RFC-0008: Progress-Aware CPM Scheduling** (2026-01-15)
   - Status date resolution chain: CLI `--as-of` > `project.status_date` > `today()`
   - Task state classification: Complete/InProgress/NotStarted via `ProgressState` enum
   - Progress-aware forward pass respects completion status
   - `Task.explicit_remaining` field for user override of remaining work
   - P005 diagnostic: remaining vs complete% conflicts
   - P006 diagnostic: container progress mismatch (>10% threshold)
   - 12 progress-aware CPM tests in `crates/utf8proj-solver/tests/progress_aware_cpm.rs`
   - Full RFC in `docs/rfc/RFC-0008-PROGRESS-AWARE-CPM.md`

2. **GitHub Actions Release Workflow** (2026-01-14)
   - Added `.github/workflows/release.yml` for cross-platform binary distribution
   - Tag-triggered releases on `vX.Y.Z` semantic version tags
   - Builds for: Windows (x86_64-pc-windows-msvc), Linux (x86_64-unknown-linux-gnu), macOS Intel (x86_64-apple-darwin), macOS ARM (aarch64-apple-darwin)
   - Produces `utf8proj` CLI and `utf8proj-lsp` binaries
   - SHA256 checksums for integrity verification
   - Unsigned binaries (matches ripgrep/bat/fd convention)
   - Fixed: `Cargo.lock` now tracked (was previously gitignored, required for `--locked` builds)
   - First release: v0.2.0 at https://github.com/alanbld/utf8proj/releases/tag/v0.2.0

2. **RFC-0003: Deterministic Resource Leveling** (2026-01-08)
   - Implemented Phase 1 of RFC-0003 in `crates/utf8proj-solver/src/leveling.rs`
   - Added `LevelingOptions`, `LevelingStrategy`, `LevelingReason`, `LevelingMetrics` types
   - Deterministic conflict sorting: (resource_id, start_date), task_id tie-breaker
   - `LevelingResult` preserves `original_schedule` for audit trail
   - Added L001-L004 diagnostic codes for leveling decisions
   - CLI `--max-delay-factor` option for limiting project extension
   - 20 leveling tests including determinism, no-op, critical path preservation

2. **Calendar Diagnostics (C001-C023)** (2026-01-07)
   - `CalendarImpact` struct tracking working days vs calendar days per task
   - 23 calendar-specific diagnostic codes
   - `filter_task_diagnostics()` for diagnostic→task linking
   - Excel export with Calendar Analysis and Diagnostics sheets
   - LSP hover with CalendarImpact display
   - Dashboard calendar visualization

3. **README Rewrite** (2026-01-08)
   - Updated tagline to "Explainable Project Scheduling"
   - Fixed broken badges and documentation links
   - Added "Why utf8proj?" comparison table
   - Linked to EXPLAINABILITY.md manifesto
   - Updated library examples to use correct CpmSolver API

4. **RFC-0005 Status & LSP Leveling Hover** (2026-01-09)
   - Created `docs/rfc/RFC-0005-RESOURCE-LEVELING-STATUS.md`: Phase 1 complete, Phase 2 deferred
   - Added L001-L004 diagnostics to LSP hover with ⚖️ icon
   - Added 📆 icon for calendar diagnostics in hover
   - Leveling coverage improved: 85.7% → 93.2%
   - Added 10 new tests (5 leveling, 5 LSP hover)

5. **v1 Editor Support Complete** (2026-01-09)
   - TextMate grammar for .proj syntax highlighting (`syntax/utf8proj.tmLanguage.json`)
   - Vim syntax file for Neovim/Vim (`syntax/proj.vim`)
   - LSP navigation: go-to-definition and find-references
   - Supported symbols: tasks, resources, calendars, profiles, traits
   - Setup instructions in `docs/EDITOR_SETUP.md`
   - Neovim: Full LSP support (syntax + diagnostics + hover + navigation)
   - Traditional Vim: Syntax highlighting only (no built-in LSP)
   - 7 navigation tests added

6. **MS Project Companion Tool: Effort Extraction & Unit Fix** (2026-01-09)
   - Added Work field extraction to `tools/mpp_to_proj/mpp_to_proj.py`
   - Maps MS Project "Work" → utf8proj `effort:` property
   - **Bug fix**: MPXJ returns units as lowercase 'h' not 'HOURS' - hours now correctly converted to days (÷8)
   - 8 unit tests covering effort extraction and unit conversion
   - 29 tests total, 99% coverage

7. **Fix: `fix container-deps` Preserves Effort Values** (2026-01-09)
   - Bug: `serialize_task()` used `else if` for effort, so effort was only written if duration was absent
   - Fix: Changed to separate `if` statements so both duration and effort are preserved
   - Location: `crates/utf8proj-cli/src/main.rs` lines 1183-1190
   - 3 TDD tests added: `crates/utf8proj-cli/tests/fix_command.rs`
   - CLI tests: 72 → 75

8. **Fix: Mermaid Renderer Hierarchical Task Names** (2026-01-09)
   - Bug: Nested tasks showed full path IDs (e.g., `task_2007.task_2014.task_2250`) instead of names
   - Root cause: `get_task()` matches on `task.id` but schedule uses full paths
   - Fix: Extract leaf ID using `rsplit('.')` before lookup
   - Location: `crates/utf8proj-render/src/mermaid.rs`
   - Mermaid now correctly shows `[task_id] Task Name` with `-V` flag

9. **Fix: mpp_to_proj Dependency Type Detection** (2026-01-10)
   - Bug: SS/FF/SF dependencies from MPP files were converted as FS (finish-to-start)
   - Root cause: MPXJ returns "SS", "FF", "SF" but converter checked for "START_START" etc.
   - Fix: Check for both formats using `.upper()` in `tools/mpp_to_proj/mpp_to_proj.py`
   - Impact: M2C project ABL stream now schedules correctly with SS dependencies

10. **Fix: Grammar Support for Dependency Type + Lag** (2026-01-10)
    - Bug: Grammar only allowed type OR lag, not both (e.g., `depends: task SS +5d` failed)
    - Fix: Changed grammar from `dep_modifier?` to `dep_type? ~ dep_lag?`
    - Location: `crates/utf8proj-parser/src/native/grammar.pest`, `native/mod.rs`
    - Now supports: `depends: task SS +5d`, `depends: task FF -2d`, etc.

11. **RFC-0004: Progressive Resource Refinement Grammar** (`crates/utf8proj-parser/src/native/grammar.pest`)
   - Added `resource_profile` declaration with specializes, skills, traits, rate ranges
   - Added `trait` declaration with description and rate_multiplier
   - Added rate range block syntax (min/max/currency) for profiles and resources
   - Added quantified assignment syntax: `assign: developer * 2`
   - Extended resource_ref to support both percentage (@50%) and quantity (*N)
   - Full RFC in `docs/rfc/RFC-0004-PROGRESSIVE-RESOURCE-REFINEMENT.md`

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

## Internal Beta Tester Project

The `personal/` directory (gitignored) contains the **TTG Migration Stream** project — an internal/private early adopter of utf8proj for real-world validation.

| Aspect | TTG Migration Stream | utf8proj |
|--------|---------------------|----------|
| Status | Internal/Private | Open Source |
| Location | `personal/` (gitignored) | Public repo |
| Role | Early adopter / Beta tester | Tool being validated |
| Data | Confidential project planning | No project data included |

**Value flow:** Real-world usage on TTG surfaces bugs and improvements → fixes committed to utf8proj → benefits all open source users.

The `personal/` directory has its own `.git` for private versioning, completely isolated from the public repository. See `personal/README.md` for documentation.

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

- CLI test coverage (42.5% currently, target 60%+)
- Edge cases in calendar parsing
- Some resource/task attribute combinations in native parser

**Explicitly Deferred:**
- RFC-0005 Phase 2 (Resource Leveling): Deferred pending user demand (see `docs/rfc/RFC-0005-RESOURCE-LEVELING-STATUS.md`)

## Grammar Notes

### Native DSL (.proj)

**Project attributes:**
- `start:`, `end:`, `currency:`, `calendar:`, `timezone:`, `status_date:`

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
- `remaining: 5d` (explicit remaining duration, overrides calculated)

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

**Dependency syntax (TJP-specific, not native DSL):**
- `depends task` - FS (Finish-to-Start, default)
- `depends task { onstart }` - SS (Start-to-Start)
- `depends task { onend }` - FF (Finish-to-Finish)
- `depends task { gapduration 2d }` - FS with lag

**Other TJP syntax:**
- `!` prefix for sibling references: `depends !kickoff`
- Resource allocation: `allocate dev1, dev2`
- Leaves: `leaves annual 2026-03-02 - 2026-03-13`
- Holidays: `leaves holiday "Name" 2026-01-01` or `vacation 2026-01-01`
- Task naming: quoted string → `summary`, `note` → supplementary info

**String escape sequences (native DSL):**
- `\"` - escaped quote
- `\\` - escaped backslash
- `\n` - newline
- `\t` - tab
