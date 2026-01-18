# RFC-0008: Progress-Aware CPM (Phase 1)

**RFC Number:** 0008
**Status:** Implemented
**Date:** 2026-01-14
**Author:** utf8proj team
**Related:** RFC-0002 (CPM Correctness), [UTF8PROJ_DESIGN_DECISIONS.md](./UTF8PROJ_DESIGN_DECISIONS.md) Part I (A1-A5)

---

## Summary

Implement progress-aware forward pass in the CPM solver. The forward pass respects actual progress data and schedules remaining work from `status_date`, not `project.start`.

## Motivation

Current CPM schedules all tasks from project start, ignoring progress. Real projects need:
1. Completed tasks locked to actual dates
2. In-progress tasks forecast from current position
3. Future tasks scheduled from predecessor completion

## Design Clarifications (Locked)

These decisions are final and drive all implementation:

### C-01: status_date Resolution

```
--as-of CLI > project.status_date > today()
```

| Priority | Source | Example |
|----------|--------|---------|
| 1 | CLI `--as-of` | `utf8proj schedule --as-of 2026-01-20 project.proj` |
| 2 | `project.status_date` | `status_date: 2026-01-20` in .proj file |
| 3 | `today()` | Current system date |

### C-02: remaining_duration Precedence

```
complete=100% → remaining=0
explicit remaining_duration → use it
else → linear derivation: duration × (1 - complete/100)
```

When `remaining_duration` and `complete` are inconsistent, emit **P005 diagnostic** and use explicit remaining.

### C-03: Container Explicit complete Override

Explicit `complete` on container overrides derived value. If explicit differs from derived by >10%, emit **P006 warning**.

### C-04: Baseline Storage

Baseline stores per-task ES/EF internally. Grammar exposes only `project.baseline_finish` in Phase 1.

---

## Specification

### Core Types

#### Project.status_date

```rust
pub struct Project {
    // ... existing fields ...

    /// Status date for progress-aware scheduling
    /// When set, remaining work schedules from this date
    pub status_date: Option<NaiveDate>,
}
```

#### Task.explicit_remaining

```rust
pub struct Task {
    // ... existing fields ...

    /// Explicit remaining duration (overrides linear derivation)
    /// When set, takes precedence over `duration × (1 - complete/100)`
    pub explicit_remaining: Option<Duration>,
}
```

#### New Diagnostics

```rust
pub enum DiagnosticCode {
    // ... existing codes ...

    /// P005: remaining_duration conflicts with complete percentage
    P005RemainingCompleteConflict,

    /// P006: Container explicit complete differs from derived (>10%)
    P006ContainerCompleteOverride,
}
```

### CpmSolver API

```rust
impl CpmSolver {
    /// Create solver with CLI-specified status date
    /// This overrides project.status_date per C-01
    pub fn with_status_date(date: NaiveDate) -> Self {
        Self {
            status_date_override: Some(date),
            ..Default::default()
        }
    }

    /// Resolve effective status date per C-01
    fn effective_status_date(&self, project: &Project) -> NaiveDate {
        self.status_date_override
            .or(project.status_date)
            .unwrap_or_else(|| Local::now().date_naive())
    }
}
```

### Forward Pass Rules

#### Rule 1: Completed Tasks (complete = 100%)

```
forecast_start = actual_start
forecast_finish = actual_finish
remaining_duration = 0
```

Completed tasks are **locked** to their actual dates regardless of what the forward pass would compute.

#### Rule 2: In-Progress Tasks (0 < complete < 100)

```
forecast_start = actual_start
remaining = explicit_remaining OR duration × (1 - complete/100)
forecast_finish = status_date + remaining (working days)
```

In-progress tasks schedule remaining work from `status_date`.

#### Rule 3: Future Tasks (complete = 0, no actual_start)

```
forecast_start = max(predecessor_finish) + 1 (standard CPM)
forecast_finish = forecast_start + duration (working days)
remaining_duration = duration
```

Future tasks schedule normally using standard CPM forward pass.

### Grammar Extensions

```pest
// In project block
project_attr = {
    start_attr | end_attr | currency_attr | calendar_attr | timezone_attr |
    status_date_attr  // NEW
}
status_date_attr = { "status_date" ~ ":" ~ date }

// In task block
task_attr = {
    duration_attr | effort_attr | ... |
    remaining_attr  // NEW
}
remaining_attr = { "remaining" ~ ":" ~ duration_value }
```

### CLI Extensions

```
utf8proj schedule [OPTIONS] <FILE>

Options:
    --as-of <DATE>    Status date for progress-aware scheduling (YYYY-MM-DD)
                      Overrides project.status_date
```

---

## Acceptance Tests

All acceptance tests are in `crates/utf8proj-solver/tests/progress_aware_cpm.rs`.

| Test | Description | Status |
|------|-------------|--------|
| test_01 | Completed task locks to actual dates | **PASSES** |
| test_02 | Partial progress schedules from status_date | Requires impl |
| test_02b | Explicit remaining takes precedence | Requires impl |
| test_03 | Future tasks schedule from predecessors | Requires impl |
| test_04 | Dependency chain with mixed progress | Requires impl |
| test_05 | Container weighted rollup | **PASSES** |
| test_06 | status_date from project field | Requires impl |
| test_06b | CLI --as-of override | Requires impl |
| test_07 | remaining vs complete conflict (P005) | Requires impl |
| test_07b | complete=100% forces remaining=0 | **PASSES** |
| test_08 | Container explicit complete (P006) | Requires impl |
| test_08b | Container explicit within threshold | Requires impl |

**Current state:** 3 tests pass (existing functionality), 9 require implementation.

---

## Implementation Checklist

### Phase A: Core Types (utf8proj-core)

- [x] Add `Project.status_date: Option<NaiveDate>`
- [x] Add `Task.explicit_remaining: Option<Duration>`
- [x] Add `DiagnosticCode::P005RemainingCompleteConflict`
- [x] Add `DiagnosticCode::P006ContainerCompleteOverride`
- [x] Update `Project::new()` defaults
- [x] Update `Task::new()` defaults

### Phase B: Solver (utf8proj-solver)

- [x] Add `CpmSolver.status_date_override: Option<NaiveDate>`
- [x] Add `CpmSolver::with_status_date()` constructor
- [x] Add `effective_status_date()` method
- [x] Implement Rule 1: completed task locking
- [x] Implement Rule 2: in-progress scheduling from status_date
- [x] Ensure Rule 3: future tasks unchanged
- [x] Add P005 emission in `analyze_project()`
- [x] Add P006 emission in `analyze_project()`

### Phase C: Parser (utf8proj-parser)

- [x] Add `status_date_attr` to grammar
- [x] Add `remaining_attr` to grammar
- [x] Parse status_date in project block
- [x] Parse remaining in task block
- [x] Update serializer for round-trip

### Phase D: CLI (utf8proj-cli)

- [x] Add `--as-of` flag to schedule command
- [x] Add `--as-of` flag to gantt command
- [x] Add `--as-of` flag to check command
- [x] Pass status_date to solver

### Phase E: Test Enablement

- [x] Enable test_02 (partial progress)
- [x] Enable test_02b (explicit remaining)
- [x] Enable test_03 (future tasks)
- [x] Enable test_04 (dependency chain)
- [x] Enable test_06 (status_date field)
- [x] Enable test_06b (CLI override)
- [x] Enable test_07 (P005)
- [x] Enable test_08 (P006)
- [x] Enable test_08b (threshold)

---

## Deferred to Phase 2

1. Percentage-based dependencies (`depends: task.75%`)
2. Split in-progress tasks for leveling
3. Baseline per-task data in grammar
4. P002 diagnostic (progress regression)
5. History/snapshot system

---

## References

- [UTF8PROJ_DESIGN_DECISIONS.md](./UTF8PROJ_DESIGN_DECISIONS.md) - Part I (A1-A5)
- [DESIGN_REFINEMENT_SUMMARY.md](./DESIGN_REFINEMENT_SUMMARY.md) - Phase 1 overview
- [progress_aware_cpm.rs](../../crates/utf8proj-solver/tests/progress_aware_cpm.rs) - Acceptance tests
