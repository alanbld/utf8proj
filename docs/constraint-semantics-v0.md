# Constraint Semantics v0

**Status:** Implemented (Phases 1-3)
**Date:** 2026-01-06
**Scope:** Task scheduling constraints (temporal)

## Implementation Status

### ✅ Phase 1: Forward Pass Constraints — Complete

All floor constraints wired to forward pass:
- `MustStartOn` — ES pushed to constraint date
- `StartNoEarlierThan` — ES floor applied
- `MustFinishOn` — EF pushed to constraint date
- `FinishNoEarlierThan` — EF floor applied

**Location:** `crates/utf8proj-solver/src/lib.rs` (lines 1292-1324)

### ✅ Phase 2: Backward Pass Constraints — Complete

All ceiling constraints wired to backward pass:
- `MustStartOn` — LS capped at constraint date
- `StartNoLaterThan` — LS ceiling applied
- `MustFinishOn` — LF capped at constraint date
- `FinishNoLaterThan` — LF ceiling applied

**Also fixed:** Negative lag handling in backward pass for FS dependencies.

**Location:** `crates/utf8proj-solver/src/lib.rs` (lines 1413-1460)

### ✅ Phase 3: Feasibility Detection — Complete

- ES ≤ LS check for all tasks after both passes
- Returns `ScheduleError::Infeasible` with task details when violated
- Negative slack = infeasible constraint combination

**Location:** `crates/utf8proj-solver/src/lib.rs` (lines 1493-1502)

### ✅ Diagnostics: E003 and W005 — Complete

| Code | Severity | Description |
|------|----------|-------------|
| E003 | Error | Infeasible constraint (ES > LS) |
| W005 | Warning | Constraint reduces slack to zero |

**Locations:**
- `crates/utf8proj-core/src/lib.rs` — DiagnosticCode enum
- `crates/utf8proj-cli/src/main.rs` — E003 emission in check/schedule
- `crates/utf8proj-solver/src/lib.rs` — W005 emission in analyze_project

### 🚧 Phase 4: Diagnostics Integration — In Progress

- [x] Add constraint info to `explain()` output
- [x] Include constraint effects in LSP hover
- [ ] Show constraint conflicts in feasibility report

**Phase 4.1 (explain() enhancement) complete:**
- Added `ConstraintEffect` struct and `ConstraintEffectType` enum to utf8proj-core
- Extended `Explanation` struct with `constraint_effects: Vec<ConstraintEffect>`
- Implemented `analyze_constraint_effects()` in CpmSolver
- Effect types: `PushedStart`, `CappedLate`, `Pinned`, `Redundant`
- Added `Task::constraint()` builder method

**Location:** `crates/utf8proj-solver/src/lib.rs` (lines 72-314)

**Phase 4.2 (LSP hover) complete:**
- Task hover now shows constraint list
- With schedule: shows detailed constraint effects with visual markers:
  - 📌 Pinned (MustStartOn/MustFinishOn active)
  - ✓ Active constraint (pushed or capped dates)
  - ○ Redundant (superseded by dependencies)
  - 🔴 Made task critical (ceiling constraint reduced slack to zero)
  - ⚠️ Superseded by dependencies

**Location:** `crates/utf8proj-lsp/src/hover.rs` (lines 235-463)

---

## Overview

This document codifies the semantics for task constraints in utf8proj. It establishes what each constraint type means, when it applies, and how violations are handled.

## Design Principles

1. **Constraints are hard by default** - A violated constraint is an error, not a warning
2. **No silent failures** - Every constraint violation must produce a diagnostic
3. **Forward and backward** - Constraints affect both CPM passes where applicable
4. **Feasibility over optimization** - Detect infeasibility, don't auto-resolve

## Constraint Types

### Temporal Constraints

Six constraint types control when tasks can be scheduled:

| Constraint | Meaning | Forward Pass | Backward Pass |
|------------|---------|--------------|---------------|
| `MustStartOn(D)` | ES = D, LS = D | ES ≥ D | LS ≤ D |
| `MustFinishOn(D)` | EF = D, LF = D | EF ≥ D | LF ≤ D |
| `StartNoEarlierThan(D)` | ES ≥ D | ES ≥ D | — |
| `StartNoLaterThan(D)` | LS ≤ D | — | LS ≤ D |
| `FinishNoEarlierThan(D)` | EF ≥ D | EF ≥ D | — |
| `FinishNoLaterThan(D)` | LF ≤ D | — | LF ≤ D |

Where:
- ES = Early Start
- EF = Early Finish
- LS = Late Start
- LF = Late Finish
- D = Constraint date

### Constraint Semantics

#### MustStartOn / MustFinishOn

These are **hard constraints** that pin a task to a specific date.

```
MustStartOn(D):
  Forward:  ES = max(ES_from_deps, D)
  Backward: LS = min(LS_from_deps, D)

  If ES > D after forward pass: INFEASIBLE (dependencies push past constraint)
  If LS < D after backward pass: INFEASIBLE (successors pull before constraint)
```

A task with `MustStartOn` has **zero slack by definition** when feasible.

#### StartNoEarlierThan / FinishNoEarlierThan

These are **floor constraints** - the task cannot start/finish before the date.

```
StartNoEarlierThan(D):
  Forward:  ES = max(ES_from_deps, D)
  Backward: (no effect)

  Always feasible for this constraint alone.
```

#### StartNoLaterThan / FinishNoLaterThan

These are **ceiling constraints** - the task must start/finish by the date.

```
StartNoLaterThan(D):
  Forward:  (no effect)
  Backward: LS = min(LS_from_deps, D)

  If ES > D after forward pass: INFEASIBLE (cannot meet deadline)
```

### Constraint Interactions

When multiple constraints apply to the same task:

```
Effective ES = max(ES_from_deps, MustStartOn, StartNoEarlierThan)
Effective LS = min(LS_from_deps, MustStartOn, StartNoLaterThan)
Effective EF = max(EF_from_deps, MustFinishOn, FinishNoEarlierThan)
Effective LF = min(LF_from_deps, MustFinishOn, FinishNoLaterThan)
```

Feasibility check:
```
ES ≤ LS  (otherwise infeasible)
EF ≤ LF  (otherwise infeasible)
```

## Diagnostics

### Error: Infeasible Constraint (E003)

Emitted when a constraint cannot be satisfied.

```
error[E003]: constraint cannot be satisfied
  --> project.proj
   |
   = task 'blocked' has infeasible constraints: ES (10) > LS (4), slack = -6 days
   = hint: check that constraints don't conflict with dependencies
```

### Warning: Constraint Zero Slack (W005)

Emitted when a ceiling constraint reduces slack to zero.

```
warning[W005]: constraint reduces slack to zero for task 'task_c'
  --> project.proj
   |
   = must_finish_on: 2025-01-10 makes task critical
   = hint: consider relaxing constraint or adding buffer
```

**Note:** Originally planned as W004, but W004 was already used for `ApproximateLeveling`. Implemented as W005.

## Test Coverage

All constraint scenarios covered in `crates/utf8proj-solver/tests/constraint_wiring.rs`:

| Test | Description |
|------|-------------|
| `start_no_earlier_than_pushes_es` | SNET floor constraint |
| `start_no_earlier_than_respects_dependencies` | Dependency overrides SNET |
| `finish_no_earlier_than_pushes_ef` | FNET floor constraint |
| `must_finish_on_sets_dates` | MFO pin constraint |
| `must_start_on_already_works` | MSO regression test |
| `start_no_later_than_caps_ls` | SNLT ceiling constraint |
| `finish_no_later_than_caps_lf` | FNLT ceiling constraint |
| `must_start_on_has_zero_slack` | Pin = zero slack |
| `must_finish_on_has_zero_slack` | Pin = zero slack |
| `infeasible_constraint_dependency_conflict` | E003 for MSO conflict |
| `infeasible_floor_ceiling_collapse` | E003 for SNET > SNLT |
| `infeasible_finish_before_start` | E003 for impossible window |
| `feasible_window_fits` | Tight fit succeeds |

CLI diagnostic test: `crates/utf8proj-cli/tests/fixtures/diagnostics/e003_infeasible_constraint.*`

### Phase 4 Tests (explain() enhancement)

Unit tests in `crates/utf8proj-solver/src/lib.rs`:

| Test | Description |
|------|-------------|
| `explain_task_with_temporal_constraint_shows_effects` | SNET constraint effect detection |
| `explain_task_with_pinned_constraint` | MustStartOn pinned effect |
| `explain_task_with_redundant_constraint` | Redundant constraint detection |
| `explain_task_without_constraints_has_empty_effects` | No constraints = empty effects |

### Phase 4.2 Tests (LSP hover)

Unit tests in `crates/utf8proj-lsp/src/hover.rs`:

| Test | Description |
|------|-------------|
| `hover_task_with_constraints_shows_list` | Constraints shown in hover |
| `hover_task_with_constraint_effects_pinned` | Pinned effect with 📌 marker |
| `hover_task_with_constraint_effects_redundant` | Redundant effect with ○ marker |

## Not In Scope (v0)

The following are explicitly **not** part of Constraint Semantics v0:

- **Soft constraints** - All constraints are hard
- **Constraint priorities** - No ranking between constraints
- **Auto-resolution** - No automatic constraint relaxation
- **Resource constraints** - Handled by separate leveling system
- **What-if constraints** - Separate analysis feature
- **Optimization** - No "best" schedule search

## Compatibility

### Breaking Changes

None. Existing `MustStartOn` behavior is preserved and extended.

### New Behavior

- Previously silent constraint types will now be enforced
- Infeasible schedules will error instead of silently producing invalid results

### Migration

Projects using unenforced constraints (`MustFinishOn`, etc.) may see new errors if constraints conflict with dependencies. This is correct behavior - the errors were always present, just unreported.

## Example: Feasible Constraints

```
task design:
    effort: 5d

task implement:
    effort: 10d
    depends: design
    start_no_earlier_than: 2025-02-10  # After design finishes anyway
```
Result: Valid, constraint is satisfied.

## Example: Infeasible Constraint

```
task design:
    effort: 10d

task implement:
    effort: 5d
    depends: design
    must_start_on: 2025-01-15  # Before design finishes
```
Result: E003 - earliest start is 2025-01-20, constraint requires 2025-01-15.

## Example: Multiple Constraints

```
task bounded:
    effort: 5d
    start_no_earlier_than: 2025-02-01
    finish_no_later_than: 2025-02-10
```
Result: Valid - 5 days fits in 7-day window (with 2 days slack).

```
task over_constrained:
    effort: 10d
    start_no_earlier_than: 2025-02-01
    finish_no_later_than: 2025-02-07
```
Result: E003 - 10 days cannot fit in 5-day window.
