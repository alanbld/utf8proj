# utf8proj - Project Context

## Project Overview

Rust-based project scheduling engine with CPM (Critical Path Method) solver and resource leveling. Parses TaskJuggler (.tjp) and native DSL (.proj) formats, schedules tasks, and renders output.

## Workspace Structure

```
crates/
├── utf8proj-core/      # Core types: Task, Resource, Dependency, Calendar, Schedule
├── utf8proj-parser/    # Parsers for TJP and native DSL (pest grammar)
├── utf8proj-solver/    # CPM scheduler with resource leveling
│   └── src/leveling.rs # Resource over-allocation detection and resolution
├── utf8proj-render/    # Output rendering (HTML Gantt, SVG)
│   └── src/gantt.rs    # Interactive HTML Gantt chart renderer
└── utf8proj-cli/       # Command-line interface (untested)
```

## Key Features Implemented

- **Hierarchical tasks**: Nested task parsing, container date derivation (min/max of children)
- **Dependency types**: FS (default), SS (!), FF (~), SF (!~) with lag support
- **Calendars**: Working days, working hours, holidays
- **Resources**: Rate, capacity, efficiency, calendar assignment
- **Task attributes**: Priority, complete %, constraints (must_start_on)
- **Critical path**: Calculation with all dependency types
- **Resource leveling**: Automatic over-allocation detection and task shifting
- **Interactive Gantt chart**: Standalone HTML output with SVG, tooltips, zoom, dependency arrows

## Test Coverage (as of 2026-01-02)

| Module | Coverage |
|--------|----------|
| utf8proj-solver/leveling | 94.4% |
| utf8proj-solver | 96.3% |
| utf8proj-render | 91.0% |
| utf8proj-parser/native | 91.2% |
| utf8proj-parser/tjp | 78.8% |
| utf8proj-core | 77.0% |
| utf8proj-cli | 0% |
| **Overall** | **81.25%** |

**Tests:** 124 passing, 1 ignored (render doctest)

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

## Recent Work Completed

1. **Interactive Gantt Chart** (`crates/utf8proj-render/src/gantt.rs`)
   - `HtmlGanttRenderer` - Generates standalone HTML with embedded SVG
   - Dependency arrows with curved paths
   - Tooltips and zoom controls
   - Light and dark themes
   - 14 tests (unit + integration)

2. **Resource Leveling** (`crates/utf8proj-solver/src/leveling.rs`)
   - `ResourceTimeline` - tracks resource usage by day
   - `detect_overallocations` - finds over-allocation periods
   - `level_resources` - resolves conflicts by shifting tasks
   - 12 integration tests covering various scenarios

3. **TJP Integration Tests** (`crates/utf8proj-solver/tests/hierarchical_scheduling.rs`)
   - `schedule_matches_tj3_output` - Parses ttg_02_deps.tjp, schedules, verifies dates
   - `schedule_ttg_hierarchy` - Tests hierarchical TJP file parsing

4. **Native Parser Coverage** (`crates/utf8proj-parser/src/native/mod.rs`)
   - Added 11 tests covering: calendar parsing, project attributes, resource attributes, task constraints, dependency lag/types, resource ref percentages, hours duration, syntax errors

## Important Files

- `crates/utf8proj-render/src/gantt.rs` - Interactive HTML Gantt chart renderer
- `crates/utf8proj-solver/src/leveling.rs` - Resource leveling algorithm
- `crates/utf8proj-solver/src/lib.rs` - CPM scheduler
- `crates/utf8proj-parser/src/native/mod.rs` - Native DSL parser
- `crates/utf8proj-parser/src/tjp/mod.rs` - TaskJuggler parser
- `crates/utf8proj-core/src/lib.rs` - Core types and traits

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
```

## Remaining Work

- CLI test coverage (0% currently)
- Edge cases in calendar parsing (lines 312-316, 326, 329)
- Some resource/task attribute combinations in native parser
- Error handling paths in leveling

## Grammar Notes

- Holiday date range uses `..` not `-`: `holiday "Name" 2025-12-25..2025-12-26`
- Resource percentage uses `@`: `assign: dev@50%`
- Constraints: `must_start_on: 2025-02-01`
- Dependency types: `depends: a` (FS), `depends: !a` (SS), `depends: a~` (FF), `depends: !a~` (SF)
- Dependency lag: `depends: a +2d` or `depends: a -1d`
