# utf8proj - Project Context

## Project Overview

Rust-based **explainable** project scheduling engine with CPM (Critical Path Method) solver and deterministic resource leveling. Parses TaskJuggler (.tjp) and native DSL (.proj) formats, schedules tasks, and renders output. Core philosophy: **"describe, don't prescribe"** — diagnostics explain scheduling decisions rather than silently fixing them.

## Versioning Policy

This project follows **Semantic Versioning** (SemVer):

| Change Type | Version Bump | Example |
|-------------|--------------|---------|
| Breaking API change | MAJOR (X.0.0) | Remove public function, change signature |
| New feature (backward compatible) | MINOR (0.X.0) | Add new diagnostic, new render format |
| Bug fix (backward compatible) | PATCH (0.0.X) | Fix calculation bug, auto-fit issue |

**IMPORTANT**: Every commit that changes behavior MUST bump the version:
- Bug fixes → bump PATCH (e.g., 0.9.0 → 0.9.1)
- New features → bump MINOR (e.g., 0.9.1 → 0.10.0)
- Breaking changes → bump MAJOR (requires discussion)

Version is set in `Cargo.toml` under `[workspace.package]`:
```toml
version = "0.15.1"
```

All crates inherit this version via `version.workspace = true`.

## Minimal Viable Feature (MVF) Protocol

**Before proposing any new feature implementation:**

1. Ask: "What's the SMALLEST version that validates the concept?"
2. Exclude by default: Interactivity, multiple formats, configuration, metrics
3. Target: Single function, ASCII output, 10-15 tests, 1-week scope
4. Future-proof: Note excluded features for separate RFCs

**Confidence Protocol for >1 week features:**
- **[Confidence: Low]** → "This seems complex. Can we ship 20% in 1 week to validate?"
- **[Confidence: Medium]** → Propose MVF first, full feature as follow-up RFC
- **[Confidence: High]** → Still ask: "Is 1-week MVP possible?"

**Anti-pattern:** `User asks X → Propose X + Y + Z + UI + metrics` (scope creep)
**Correct:** `User asks X → Propose minimal X → Exclude Y,Z for future RFCs`

## Workspace Structure

```
crates/
├── utf8proj-core/      # Core types: Task, Resource, Dependency, Calendar, Schedule
├── utf8proj-parser/    # Parsers for TJP and native DSL (pest grammar)
├── utf8proj-solver/    # CPM scheduler with deterministic resource leveling
├── utf8proj-render/    # Output rendering (HTML, SVG, Mermaid, PlantUML, Excel)
├── utf8proj-cli/       # Command-line interface
├── utf8proj-lsp/       # Language Server Protocol implementation
└── utf8proj-wasm/      # WebAssembly bindings for browser playground

playground/             # Browser-based playground (Monaco editor, WASM)
syntax/                 # Editor syntax highlighting (TextMate, Vim)
tools/mpp_to_proj/      # MS Project companion tool (Python, handles Manually Scheduled tasks)
tools/benchmarks/       # PSPLIB benchmark runner (RFC-0015 validation)
tools/psplib_to_proj.py # PSPLIB → .proj converter
docs/                   # Documentation (MS_PROJECT_COMPARISON.md, EDITOR_SETUP.md, RFCs)
.github/workflows/      # Release workflow (cross-platform binaries)
```

## Key Features Implemented

- **Hierarchical tasks**: Nested task parsing, container date derivation (min/max of children)
- **Dependency types**: FS (default), SS, FF, SF with lag support (+2d, -1d)
- **Calendars**: Working days, working hours, holidays (single-date and range)
- **Resources**: Rate, capacity, efficiency, calendar, email, role, leave
- **Task attributes**: Priority, complete %, constraints, note, tag, cost, payment
- **Milestones**: Dedicated `milestone` declaration syntax; ignore working day rules (can occur on weekends/holidays)
- **Constraints**: Declarative constraint blocks for what-if analysis
- **Critical path**: Calculation with all dependency types
- **Effort-driven scheduling**: PMI-compliant Duration = Effort / Resource_Units
- **Resource leveling**: RFC-0003 deterministic leveling with full audit trail (L001-L004 diagnostics)
- **Hybrid BDD leveling**: RFC-0014 cluster-based leveling (4-5x faster for large projects via `--leveling-strategy=hybrid`)
- **Progress-aware scheduling**: RFC-0008 status date resolution, remaining duration calculation, P005/P006 diagnostics
- **Temporal regimes**: RFC-0012 work/event/deadline modes for calendar interaction (`regime: event` allows weekend scheduling)
- **Calendar diagnostics**: C001-C023 codes for working days vs calendar days analysis
- **BDD conflict analysis**: Binary Decision Diagram-based conflict detection (experimental)
- **Focus view**: RFC-0006 pattern-based filtering for large Gantt charts (`--focus`, `--context-depth`)
- **Baseline management**: RFC-0013 schedule snapshots with variance analysis (`baseline save/list/compare`)
- **Multiple render formats**: HTML, SVG, MermaidJS, PlantUML, Excel (XLSX)
- **Excel progress tracking**: RFC-0018 progress columns, visual formatting, status icons, variance
- **Now line rendering**: RFC-0017 vertical status date marker on Gantt charts (all formats)
- **Browser playground**: WASM-based in-browser scheduler with Monaco editor, Excel options panel

## Test Coverage

~950 tests (including 60 E2E), ~86% overall coverage. All core business logic components achieve 90%+ coverage (excluding CLI entry point at 42.5%).

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
| L001-L004 | Info/Warning | Leveling decisions (resolved, unresolvable, duration increase, milestone delay) |
| P005-P006 | Warning | Progress conflicts (remaining vs complete%, container mismatch) |
| C001-C023 | Various | Calendar impact (working days vs calendar days) |
| B001 | Info | Baseline saved successfully |
| B002 | Warning | Task lacks explicit ID (using inferred) |
| B003 | Error | Baseline already exists |
| B004 | Error | Baseline not found |
| B005-B006 | Info | Task removed/added since baseline |
| B007 | Warning | No baselines file found |
| B008 | Warning | Container excluded from baseline |
| B009 | Error | Cannot baseline: tasks have no ID |

## Language Server Protocol (LSP)

Provides IDE support for `.proj` files: real-time diagnostics, hover info (profiles, resources, tasks, traits, leveling/calendar diagnostics), document symbols, go-to-definition, and find-references. See `docs/EDITOR_SETUP.md` for editor configuration (Neovim recommended for full experience).

## Scheduling Features

### Effort-Driven Scheduling (PMI Compliant)
```
Duration = Effort / Total_Resource_Units
```
Example: 40h effort with 1 resource @ 50% = 10 days. Use `assign_with_units("dev", 0.5)` for partial allocations.

### Resource Leveling (RFC-0003, RFC-0014)
- **Opt-in only**: `CpmSolver::with_leveling()` or `-l` CLI flag
- **Deterministic**: Same inputs → identical outputs (stable sorting)
- **Auditable**: Every shift has a `LevelingReason`
- **L001-L004 diagnostics** explain all leveling decisions
- **Leveling strategies** (`--leveling-strategy`):
  - `critical-path-first` (default): Priority-based heuristic
  - `hybrid`: BDD cluster analysis + heuristic (4-5x faster for large projects)
  - `optimal`: Branch-and-bound solver (experimental, small projects only)

### Progress-Aware Scheduling (RFC-0008)
- **Status Date Resolution**: CLI `--as-of` > `project.status_date` > `today()`
- **Task States**: Complete (100%, locked), InProgress (remaining work from status date), NotStarted
- **Remaining Duration**: `duration * (1 - complete%)` or explicit `remaining:` override

## Render Formats

| Format | CLI Flag | Key Features |
|--------|----------|--------------|
| HTML/SVG | `-f html` (default) | Interactive: zoom, tooltips, dependency arrows, dark theme, now line |
| MermaidJS | `-f mermaid` | Markdown-embeddable, critical path markers, section grouping, todayMarker |
| PlantUML | `-f plantuml` | Wiki-friendly, scale options, status date coloring |
| Excel | `-f xlsx` | Formula-driven scheduling, VLOOKUP dependencies, cost sheets, progress tracking |

All renderers support: `--focus="pattern"`, `--context-depth=N`, `-V` (verbose), `--task-ids`, `-w` (label width).

**Excel Progress Tracking (RFC-0018)**: Use `--progress MODE` with Excel exports:
- `none` - Clean schedule view (default)
- `columns` - Adds Complete%, Remaining, Actual Start/End columns
- `visual` - Color-coded timeline (green=done, red=behind, blue=remaining)
- `full` - Status icons (✓●○⚠), variance column, full tracking

**Now Line (RFC-0017)**: All Gantt renderers show a vertical line at the status date. CLI flags:
- `--as-of DATE` - Override status date
- `--no-now-line` - Disable now line
- `--show-today` - Show today line separately when status_date differs

## Important Files

- `crates/utf8proj-solver/src/lib.rs` - CPM scheduler with effort-driven calculation
- `crates/utf8proj-solver/src/leveling.rs` - Resource leveling algorithm (heuristic + hybrid BDD)
- `crates/utf8proj-solver/src/optimal.rs` - Optimal leveling solver (experimental)
- `crates/utf8proj-solver/src/bdd.rs` - BDD-based conflict cluster analysis
- `crates/utf8proj-parser/src/native/grammar.pest` - Native DSL grammar
- `crates/utf8proj-render/src/gantt.rs` - Interactive HTML Gantt chart renderer
- `crates/utf8proj-render/src/excel.rs` - Excel costing report with dependencies
- `docs/SCHEDULING_ANALYSIS.md` - PMI/PERT/CPM compliance analysis
- `docs/MS_PROJECT_COMPARISON.md` - Feature comparison with MS Project
- `docs/rfc/RFC-0014-SCALING-RESOURCE-LEVELING.md` - Hybrid BDD leveling design
- `docs/rfc/RFC-0015-BENCHMARKING-VALIDATION.md` - PSPLIB benchmark framework
- `docs/rfc/RFC-0017-NOW-LINE.md` - Now line rendering for Gantt charts
- `docs/rfc/RFC-0018-EXCEL-PROGRESS-TRACKING.md` - Excel progress columns and visual formatting

## MS Project Compatibility

The `tools/mpp_to_proj/` companion tool converts MS Project files (.mpp) to utf8proj format:

```bash
python3 tools/mpp_to_proj/mpp_to_proj.py project.mpp project.proj
utf8proj fix container-deps project.proj -o project_fixed.proj  # Optional: inherit container deps
utf8proj schedule project_fixed.proj
```

**Validated conversions** produce identical schedules to MS Project. Key handling:
- Manually Scheduled tasks → `must_start_on:` constraints
- All dependency types (FS, SS, FF, SF) with lag
- Container dependency inheritance via `fix container-deps`

See `docs/MS_PROJECT_COMPARISON.md` for full feature comparison.

## Example Projects

```
examples/
├── crm_migration.proj   # Full-featured CRM project (native DSL)
├── crm_migration.tjp    # TaskJuggler equivalent
├── crm_simple.proj      # Simplified version for testing
└── crm_simple.tjp       # Simplified TJP version
```

## Internal Beta Tester

The `personal/` directory (gitignored) contains the **TTG Migration Stream** project — an internal beta tester for real-world validation. Value flow: Real-world usage surfaces bugs → fixes benefit all users.

## Commands

```bash
# Run all tests
cargo test --workspace

# Check coverage
cargo tarpaulin --workspace --out Stdout --skip-clean

# Build release
cargo build --release

# Validate a project (fast, no output)
utf8proj check project.proj
utf8proj check --strict project.proj    # warnings→errors
utf8proj check --format=json project.proj

# Schedule a project
utf8proj schedule project.tjp
utf8proj schedule -l project.tjp                              # with resource leveling
utf8proj schedule -l --leveling-strategy=hybrid project.tjp   # hybrid BDD leveling (faster)

# Generate Gantt chart
utf8proj gantt project.tjp -o chart.svg              # SVG (default)
utf8proj gantt project.tjp -o chart.mmd -f mermaid   # MermaidJS
utf8proj gantt project.tjp -o chart.puml -f plantuml # PlantUML
utf8proj gantt project.tjp -o chart.xlsx -f xlsx     # Excel workbook
utf8proj gantt project.tjp -o chart.xlsx --currency EUR --weeks 40

# Excel progress tracking (RFC-0018)
utf8proj gantt project.proj -o out.xlsx -f xlsx --progress none     # Clean view
utf8proj gantt project.proj -o out.xlsx -f xlsx --progress columns  # Data columns
utf8proj gantt project.proj -o out.xlsx -f xlsx --progress visual   # Color-coded
utf8proj gantt project.proj -o out.xlsx -f xlsx --progress full     # Full tracking

# Now line options (RFC-0017)
utf8proj gantt project.proj --as-of 2026-01-20       # Override status date
utf8proj gantt project.proj --no-now-line            # Disable now line
utf8proj gantt project.proj --show-today             # Show both status date and today

# Baseline management (RFC-0013)
utf8proj baseline save --name original project.proj   # Save baseline
utf8proj baseline list project.proj                   # List baselines
utf8proj baseline show --name original project.proj   # Show baseline details
utf8proj baseline remove --name old project.proj      # Remove baseline
utf8proj compare --baseline original project.proj     # Compare vs baseline
utf8proj compare --baseline original --format json project.proj  # JSON output

# Run benchmarks
utf8proj benchmark -t chain -c 10000 --series
utf8proj bdd-benchmark --series

# PSPLIB benchmarks (RFC-0015)
cd tools/benchmarks && ./run_psplib.sh   # requires PSPLIB dataset
python3 tools/psplib_to_proj.py j30.sm/j301_1.sm -o project.proj

# Build WASM playground
cd playground && ./build.sh && python3 -m http.server 8080
```

## Clippy Lint Configuration

Strict linting with `pedantic` and `nursery` enabled. CI runs `cargo clippy --workspace --all-targets -- -D warnings`. Config in root `Cargo.toml` under `[workspace.lints.clippy]`. Crates inherit via `[lints] workspace = true`. Use underscore separators for new lint exceptions.

## Remaining Work

- CLI test coverage (42.5% currently, target 60%+)
- Edge cases in calendar parsing
- Some resource/task attribute combinations in native parser

**Explicitly Deferred:**
- RFC-0005 Phase 2 (Resource Leveling): Deferred pending user demand

**Changelog:** See `CHANGELOG.md` for detailed history.

## Grammar Notes

### Native DSL (.proj)

**Project attributes:**
- `start:`, `end:`, `currency:`, `calendar:`, `timezone:`
- `status_date:` - Reporting "as-of" date for progress calculations and now line (RFC-0017)

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
- `regime: work | event | deadline` (temporal regime for calendar interaction)

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

**Temporal Regimes (RFC-0012):**
```proj
task work_task "Work Task" { regime: work }      # Default: respects working days
milestone release "Release" { regime: event }    # Can occur on weekends/holidays
milestone deadline "Deadline" { regime: deadline } # Exact date required
```

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
