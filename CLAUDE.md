# utf8proj - Project Context

## Project Overview

Rust-based project scheduling engine with CPM (Critical Path Method) solver. Parses TaskJuggler (.tjp) and native DSL (.proj) formats, schedules tasks, and renders output.

## Workspace Structure

```
crates/
├── utf8proj-core/      # Core types: Task, Resource, Dependency, Calendar, Schedule
├── utf8proj-parser/    # Parsers for TJP and native DSL (pest grammar)
├── utf8proj-solver/    # CPM scheduler with hierarchical task support
├── utf8proj-render/    # Output rendering (TJP format)
└── utf8proj-cli/       # Command-line interface (untested)
```

## Key Features Implemented

- **Hierarchical tasks**: Nested task parsing, container date derivation (min/max of children)
- **Dependency types**: FS (default), SS (!), FF (~), SF (!~) with lag support
- **Calendars**: Working days, working hours, holidays
- **Resources**: Rate, capacity, efficiency, calendar assignment
- **Task attributes**: Priority, complete %, constraints (must_start_on)
- **Critical path**: Calculation with all dependency types

## Test Coverage (as of 2026-01-02)

| Module | Coverage |
|--------|----------|
| utf8proj-solver | 96.3% |
| utf8proj-render | 91.0% |
| utf8proj-parser/native | 91.2% |
| utf8proj-parser/tjp | 78.8% |
| utf8proj-core | 74.6% |
| utf8proj-cli | 0% |
| **Overall** | **78.89%** |

**Tests:** 80 passing, 1 ignored (render doctest)

## Recent Work Completed

1. **TJP Integration Tests** (`crates/utf8proj-solver/tests/hierarchical_scheduling.rs`)
   - `schedule_matches_tj3_output` - Parses ttg_02_deps.tjp, schedules, verifies dates
   - `schedule_ttg_hierarchy` - Tests hierarchical TJP file parsing

2. **Container Date Derivation Tests** (`crates/utf8proj-parser/tests/hierarchical_tasks.rs`)
   - `container_start_is_min_of_children`
   - `container_finish_is_max_of_children`

3. **Native Parser Coverage** (`crates/utf8proj-parser/src/native/mod.rs`)
   - Added 11 tests covering: calendar parsing, project attributes, resource attributes, task constraints, dependency lag/types, resource ref percentages, hours duration, syntax errors

## Important Files

- `crates/utf8proj-parser/src/native/mod.rs` - Native DSL parser
- `crates/utf8proj-parser/src/tjp/mod.rs` - TaskJuggler parser
- `crates/utf8proj-solver/src/lib.rs` - CPM scheduler
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
- Error handling paths

## Grammar Notes

- Holiday date range uses `..` not `-`: `holiday "Name" 2025-12-25..2025-12-26`
- Resource percentage uses `@`: `assign: dev@50%`
- Constraints: `must_start_on: 2025-02-01`
- Dependency types: `depends: a` (FS), `depends: !a` (SS), `depends: a~` (FF), `depends: !a~` (SF)
- Dependency lag: `depends: a +2d` or `depends: a -1d`
