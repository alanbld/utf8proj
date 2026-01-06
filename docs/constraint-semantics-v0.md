# Constraint Semantics v0

**Status:** Draft
**Date:** 2026-01-06
**Scope:** Task scheduling constraints (temporal)

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
  --> project.proj:15
   |
15 |     must_start_on: 2025-02-01
   |     ^^^^^^^^^^^^^^^^^^^^^^^^^ task 'implement' must start on 2025-02-01
   |
   = note: earliest possible start is 2025-02-10 (due to dependency on 'design')
   = note: constraint is infeasible by 7 working days
```

### Warning: Constraint Narrows Slack (W004)

Emitted when a constraint significantly reduces scheduling flexibility.

```
warning[W004]: constraint reduces slack to zero
  --> project.proj:15
   |
15 |     start_no_later_than: 2025-02-15
   |     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ task 'implement' now has 0 days slack
   |
   = note: task is now on critical path due to constraint
   = hint: consider relaxing constraint or adding buffer
```

### Info: Constraint Applied (I002)

Available in verbose mode to show constraint effects.

```
info[I002]: constraint applied
  --> project.proj:15
   |
   = task 'implement' start constrained to 2025-02-01
   = effective slack: 3 days
```

## Implementation Phases

### Phase 1: Forward Pass Constraints (Current + Enhancement)

Wire all "floor" constraints to forward pass:
- `MustStartOn` ✅ (exists)
- `StartNoEarlierThan` (to wire)
- `MustFinishOn` → affects EF (to wire)
- `FinishNoEarlierThan` → affects EF (to wire)

### Phase 2: Backward Pass Constraints

Wire all "ceiling" constraints to backward pass:
- `MustStartOn` → also constrain LS (to wire)
- `StartNoLaterThan` → constrain LS (to wire)
- `MustFinishOn` → also constrain LF (to wire)
- `FinishNoLaterThan` → constrain LF (to wire)

### Phase 3: Feasibility Detection

After both passes:
1. Check ES ≤ LS for all tasks
2. Check EF ≤ LF for all tasks
3. Emit E003 for violations
4. Emit W004 for zero-slack from constraints

### Phase 4: Diagnostics Integration

- Add constraint info to `explain()` output
- Include constraint effects in hover (LSP)
- Show constraint conflicts in feasibility report

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

## Test Cases

### Feasible Constraints

```
task design:
    effort: 5d

task implement:
    effort: 10d
    depends: design
    start_no_earlier_than: 2025-02-10  # After design finishes anyway
```
Result: Valid, constraint is satisfied.

### Infeasible Constraint

```
task design:
    effort: 10d

task implement:
    effort: 5d
    depends: design
    must_start_on: 2025-01-15  # Before design finishes
```
Result: E003 - earliest start is 2025-01-20, constraint requires 2025-01-15.

### Ceiling Constraint Violation

```
task critical:
    effort: 20d
    start_no_later_than: 2025-01-10

# Project starts 2025-01-06
```
Result: E003 - task needs 20 days but must start by day 4.

### Multiple Constraints

```
task bounded:
    effort: 5d
    start_no_earlier_than: 2025-02-01
    finish_no_later_than: 2025-02-10
```
Result: Valid - 5 days fits in 7-day window.

```
task over_constrained:
    effort: 10d
    start_no_earlier_than: 2025-02-01
    finish_no_later_than: 2025-02-07
```
Result: E003 - 10 days cannot fit in 5-day window.
