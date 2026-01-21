# Changelog

All notable changes to utf8proj are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.11.1] - 2026-01-21

### Fixed
- **Resource leveling infinite loop** — `find_available_slot()` had an unbounded loop that would hang forever when no slot could be found (e.g., when task units exceed resource capacity). Real project files with resource assignments would hang while synthetic benchmarks worked.
  - Added 2000 working day search limit to prevent infinite loops
  - Function now returns `Option<NaiveDate>` to handle "no slot found" gracefully
  - Early exit when `units > capacity` (impossible to schedule)
  - Emits L002 diagnostic when slot cannot be found within search horizon

### Added
- **Stress test examples** for performance validation:
  - `examples/acso_tutorial.proj` — TaskJuggler tutorial equivalent (15 tasks, 6 resources)
  - `examples/enterprise_10k.proj` — Enterprise-scale project (10k tasks, 1k resources)
  - `tools/generate_large_project.py` — Python generator for custom stress tests

### Performance
- Leveling now completes for real projects that previously hung:
  - `acso_tutorial.proj` (15 tasks): hung → **0.3s**
  - `crm_simple.proj` (28 tasks): hung → **0.1s**
  - 1000 tasks with multi-assignment: hung → **0.4s**
- Scheduling (without leveling) scales to 1M+ tasks

## [0.11.0] - 2026-01-20

### Added
- **RFC-0013: Baseline Management** — Schedule snapshots for variance analysis
  - Capture frozen schedule snapshots to answer "Compared to what?"
  - Two-file architecture: `project.proj` + `project.proj.baselines` (sidecar)
  - Only leaf tasks are baselined (containers excluded to prevent double-counting)
  - Immutable baselines (no `--force` flag — delete and recreate instead)
  - Fully-qualified task IDs (e.g., `phase1.design`) prevent collisions
  - New CLI commands:
    - `utf8proj baseline save --name <name>` — Save a baseline snapshot
    - `utf8proj baseline list` — List all baselines for a project
    - `utf8proj baseline show --name <name>` — Show baseline details
    - `utf8proj baseline remove --name <name>` — Remove a baseline (with confirmation)
    - `utf8proj compare --baseline <name>` — Compare current schedule vs baseline
  - Output formats: text (default), CSV (`--format csv`), JSON (`--format json`)
  - Filtering options: `--show-unchanged`, `--threshold <days>`
  - New diagnostics: B001-B009 for baseline operations
  - Full RFC in `docs/rfc/RFC-0013-baseline-management.md`

### New Diagnostics

| Code | Severity | Meaning |
|------|----------|---------|
| B001 | Info | Baseline saved successfully |
| B002 | Warning | Task lacks explicit ID (using inferred) |
| B003 | Error | Baseline already exists |
| B004 | Error | Baseline not found |
| B005 | Info | Task removed since baseline |
| B006 | Info | Task added since baseline |
| B007 | Warning | No baselines file found |
| B008 | Warning | Container excluded from baseline |
| B009 | Error | Cannot baseline: tasks have no ID |

### Example Usage

```bash
# Save initial baseline
utf8proj baseline save --name original --description "Initial plan" project.proj

# Make changes to project, then compare
utf8proj compare --baseline original project.proj

# Output:
# Schedule Variance vs "original" (saved 2026-01-15)
#
# Task                 Baseline Finish   Current Finish   Variance
# design               2026-01-10        2026-01-12       +2d !!
# build                2026-02-15        2026-02-20       +5d !!
# [+] security_audit   -                 2026-02-28       (added)
#
# Summary:
#   Compared: 2 tasks
#   Delayed: 2
#   Added: 1
#   Project slip: +7 days

# JSON output for CI integration
utf8proj compare --baseline original --format json project.proj
```

### Baseline File Format

```proj
# project.proj.baselines
baseline original {
    saved: 2026-01-15T10:30:00Z
    description: "Initial approved plan"

    design: 2026-01-01 -> 2026-01-10
    build: 2026-01-11 -> 2026-02-15
}

baseline change_order_1 {
    saved: 2026-02-01T14:20:00Z
    parent: original

    design: 2026-01-01 -> 2026-01-12
    build: 2026-01-13 -> 2026-02-20
}
```

## [0.10.0] - 2026-01-18

### Added
- **RFC-0012: Temporal Regimes** — Explicit time semantics for tasks
  - Three regimes: `Work` (effort-bearing), `Event` (point-in-time), `Deadline` (contractual)
  - New `regime:` task attribute: `regime: work`, `regime: event`, `regime: deadline`
  - `Task.effective_regime()` method derives regime from explicit setting or milestone flag
  - New diagnostics: R001-R005 for regime validation
  - `--explain` CLI flag shows detailed explanations for all diagnostic codes
  - Full RFC in `docs/rfc/RFC-0012-TEMPORAL-REGIMES.md`

### Changed
- Milestones are now treated as events with exact dates (Event regime)
- Solver no longer special-cases milestones — uses `effective_regime()` instead
- Constraint rounding is regime-driven, not calendar-driven

### Migration Notes (0.9.x → 0.10.0)

**Backward Compatible:** Existing projects work without changes.

The new Temporal Regimes feature is **opt-in**. If you don't specify `regime:`, the system uses:
- `Event` for milestones (zero-duration tasks)
- `Work` for all other tasks

**Key behavioral improvements:**
- Milestones constrained to weekends/holidays now stay on those dates (Event regime)
- Previously, milestones would round to the nearest working day

**New `regime:` syntax (optional):**
```proj
# Explicit Work regime (default for non-milestones)
task dev "Development" {
    effort: 5d
    regime: work
}

# Explicit Event regime (default for milestones)
milestone release "Release v2.0" {
    regime: event
    start_no_earlier_than: 2025-01-12  # Sunday - stays on Sunday
}

# Deadline regime (external constraints)
task contract "Contract Deadline" {
    duration: 1d
    regime: deadline
    finish_no_later_than: 2025-01-31
}
```

**New diagnostics:**
| Code | Severity | Meaning |
|------|----------|---------|
| R001 | Info | Event regime task has non-zero duration |
| R002 | Info | Work regime constraint falls on non-working day |
| R003 | Warning | Deadline regime without finish constraint |
| R004 | Info | Implicit Event regime applied to milestone |
| R005 | Info | Mixed regime dependency (informational) |

**Use `--explain` for detailed guidance:**
```bash
utf8proj check --explain project.proj
```

## [0.9.1] - 2026-01-16

### Fixed
- **Excel Auto-Fit Bug Fixes**
  - Fixed `add_schedule_sheet` to use `get_effective_weeks()` instead of `self.schedule_weeks`
  - Fixed `calculate_auto_fit_weeks` to use actual max task finish date (not just `schedule.project_end`)
  - Both fixes ensure all tasks are covered in week columns for long projects
  - Affected methods: `write_schedule_row_simple`, `write_schedule_row_with_deps`, `write_week_columns`, `write_schedule_totals`

## [0.9.0] - 2026-01-15

### Added
- **RFC-0006: Focus View for Gantt Charts**
  - `focus()` and `context_depth()` builder methods on `HtmlGanttRenderer`
  - Pattern matching: prefix, contains, glob patterns for task filtering
  - Context depth control: show ancestors/siblings of focused tasks
  - CLI options: `--focus="pattern"` and `--context-depth=N`
  - 12 focus view tests in `crates/utf8proj-render/src/gantt.rs`
  - Full RFC in `docs/rfc/RFC-0006-FOCUS-VIEW.md`

- **RFC-0008: Progress-Aware CPM Scheduling**
  - Status date resolution chain: CLI `--as-of` > `project.status_date` > `today()`
  - Task state classification: Complete/InProgress/NotStarted via `ProgressState` enum
  - Progress-aware forward pass respects completion status
  - `Task.explicit_remaining` field for user override of remaining work
  - P005 diagnostic: remaining vs complete% conflicts
  - P006 diagnostic: container progress mismatch (>10% threshold)
  - 12 progress-aware CPM tests
  - Full RFC in `docs/rfc/RFC-0008-PROGRESS-AWARE-CPM.md`

## [0.2.0] - 2026-01-14

### Added
- **GitHub Actions Release Workflow**
  - `.github/workflows/release.yml` for cross-platform binary distribution
  - Tag-triggered releases on `vX.Y.Z` semantic version tags
  - Builds for: Windows, Linux, macOS Intel, macOS ARM
  - Produces `utf8proj` CLI and `utf8proj-lsp` binaries
  - SHA256 checksums for integrity verification
  - Unsigned binaries (matches ripgrep/bat/fd convention)

### Fixed
- `Cargo.lock` now tracked (was previously gitignored, required for `--locked` builds)

## [Unreleased - 2026-01-10]

### Fixed
- **mpp_to_proj Dependency Type Detection**
  - SS/FF/SF dependencies from MPP files now correctly converted (was always FS)
  - MPXJ returns "SS", "FF", "SF" but converter checked for "START_START" etc.
  - Fix: Check for both formats using `.upper()` in `tools/mpp_to_proj/mpp_to_proj.py`

- **Grammar Support for Dependency Type + Lag**
  - Grammar now allows type AND lag together (e.g., `depends: task SS +5d`)
  - Changed grammar from `dep_modifier?` to `dep_type? ~ dep_lag?`
  - Location: `crates/utf8proj-parser/src/native/grammar.pest`

## [Unreleased - 2026-01-09]

### Added
- **v1 Editor Support**
  - TextMate grammar for .proj syntax highlighting (`syntax/utf8proj.tmLanguage.json`)
  - Vim syntax file for Neovim/Vim (`syntax/proj.vim`)
  - LSP navigation: go-to-definition and find-references
  - Supported symbols: tasks, resources, calendars, profiles, traits
  - Setup instructions in `docs/EDITOR_SETUP.md`
  - 7 navigation tests

- **RFC-0005 Status & LSP Leveling Hover**
  - `docs/rfc/RFC-0005-RESOURCE-LEVELING-STATUS.md`: Phase 1 complete, Phase 2 deferred
  - L001-L004 diagnostics in LSP hover with icons
  - Leveling coverage improved: 85.7% → 93.2%

- **MS Project Companion Tool: Effort Extraction**
  - Work field extraction in `tools/mpp_to_proj/mpp_to_proj.py`
  - Maps MS Project "Work" → utf8proj `effort:` property
  - 8 unit tests covering effort extraction

### Fixed
- **MPXJ Unit Fix**: Units returned as lowercase 'h' not 'HOURS' - hours now correctly converted to days (÷8)

- **`fix container-deps` Preserves Effort Values**
  - `serialize_task()` used `else if` for effort, so effort was only written if duration was absent
  - Changed to separate `if` statements so both duration and effort are preserved

- **Mermaid Renderer Hierarchical Task Names**
  - Nested tasks showed full path IDs instead of names
  - Fix: Extract leaf ID using `rsplit('.')` before lookup

## [Unreleased - 2026-01-08]

### Added
- **RFC-0003: Deterministic Resource Leveling**
  - Phase 1 implementation in `crates/utf8proj-solver/src/leveling.rs`
  - `LevelingOptions`, `LevelingStrategy`, `LevelingReason`, `LevelingMetrics` types
  - Deterministic conflict sorting: (resource_id, start_date), task_id tie-breaker
  - `LevelingResult` preserves `original_schedule` for audit trail
  - L001-L004 diagnostic codes
  - CLI `--max-delay-factor` option
  - 20 leveling tests

- **README Rewrite**
  - Updated tagline to "Explainable Project Scheduling"
  - Fixed broken badges and documentation links
  - Added "Why utf8proj?" comparison table
  - Linked to EXPLAINABILITY.md manifesto

## [Unreleased - 2026-01-07]

### Added
- **Calendar Diagnostics (C001-C023)**
  - `CalendarImpact` struct tracking working days vs calendar days per task
  - 23 calendar-specific diagnostic codes
  - `filter_task_diagnostics()` for diagnostic→task linking
  - Excel export with Calendar Analysis and Diagnostics sheets
  - LSP hover with CalendarImpact display

## [Unreleased - Earlier]

### Added
- **RFC-0004: Progressive Resource Refinement Grammar**
  - `resource_profile` declaration with specializes, skills, traits, rate ranges
  - `trait` declaration with description and rate_multiplier
  - Rate range block syntax (min/max/currency)
  - Quantified assignment syntax: `assign: developer * 2`
  - Extended resource_ref: percentage (@50%) and quantity (*N)

- **PMI-Compliant Effort Scheduling**
  - Fixed effort-to-duration calculation: `Duration = Effort / Total_Resource_Units`
  - `Task::assign_with_units()` for partial allocations
  - See `docs/SCHEDULING_ANALYSIS.md`

- **BDD Conflict Analysis** (`crates/utf8proj-solver/src/bdd.rs`)
  - `BddConflictAnalyzer` using Biodivine library
  - Encodes resource conflicts as Boolean satisfiability
  - 5 tests

- **CLI Enhancements**
  - `-l/--leveling` flag for `schedule` command
  - `bdd-benchmark` subcommand

- **WASM Playground** (`crates/utf8proj-wasm/`, `playground/`)
  - `Playground` struct with WASM bindings
  - Monaco editor with custom syntax highlighting
  - Real-time validation, live Gantt preview
  - Share functionality, theme toggle

- **Interactive Gantt Chart** (`crates/utf8proj-render/src/gantt.rs`)
  - Standalone HTML with embedded SVG
  - Dependency arrows, tooltips, zoom controls
  - Light and dark themes

- **Resource Leveling** (`crates/utf8proj-solver/src/leveling.rs`)
  - `ResourceTimeline`, `detect_overallocations`, `level_resources`
  - 12 integration tests

- **MermaidJS Renderer** (`crates/utf8proj-render/src/mermaid.rs`)
  - Critical path markers, milestone detection, dependency syntax
  - 12 tests

- **PlantUML Renderer** (`crates/utf8proj-render/src/plantuml.rs`)
  - Critical path coloring, dependency syntax, milestone markers
  - 17 tests

- **Excel Costing Report** (`crates/utf8proj-render/src/excel.rs`)
  - XLSX files with rust_xlsxwriter
  - Formula-driven scheduling with VLOOKUP dependencies
  - 13 tests

- **Extended Native DSL Grammar**
  - Project: `timezone:` attribute
  - Resources: `email:`, `role:`, `leave:` attributes
  - Tasks: `note:`, `tag:`, `cost:`, `payment:` attributes
  - Milestones: Dedicated `milestone id "name" { }` syntax
  - Reports, constraints, holidays (single-date support)

- **Tutorial & Benchmark Documentation** (`docs/`)
  - `tutorial.md` - CRM migration example
  - `benchmark-report.md` - TaskJuggler comparison

---

## Historical Bug Fixes

### TJP Parser: Choice Rule Unwrapping (2026-01-04)
**Location:** `crates/utf8proj-parser/src/tjp/mod.rs`

**Problem:** The `project_attr` and `resource_attr` choice rules in the pest grammar were not being unwrapped before matching. Attributes like `currency` and `efficiency` were silently ignored.

**Root Cause:** Pest returns choice rules as their container type (e.g., `project_attr`) rather than the matched alternative (e.g., `currency_attr`).

**Fix:** Added unwrapping logic:
```rust
let actual_attr = if attr.as_rule() == Rule::project_attr {
    attr.into_inner().next().unwrap()
} else {
    attr
};
```

**Tests Added:** `parse_project_currency`, `parse_resource_efficiency`, `parse_dependency_onstart`, `parse_dependency_onend`

### CPM Backward Pass for Non-FS Dependencies (2026-01-05)
**Location:** `crates/utf8proj-solver/src/lib.rs`

**Problem:** The backward pass treated all dependencies as FS, computing `LF(pred) = min(LS(succ))` regardless of dependency type. This caused incorrect slack calculations for SS/FF/SF dependencies.

**Fix:** Enhanced backward pass to look up the actual dependency type and apply the correct formula:
- **FS:** `LF(pred) <= LS(succ) - lag`
- **SS:** `LF(pred) <= LS(succ) - lag + duration(pred)`
- **FF:** `LF(pred) <= LF(succ) - lag`
- **SF:** `LF(pred) <= LF(succ) - lag + duration(pred)`

**Tests Added:** 5 tests in `crates/utf8proj-solver/tests/cpm_correctness.rs`

**Impact:** Correct slack calculations for all dependency types.
